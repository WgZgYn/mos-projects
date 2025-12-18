use crate::constant::{RAMFS_FRAME_SIZE_BYTE, RAMFS_PATH_DELIMITER, inode, structs_size};
use crate::frame::Frame;
use crate::fs::{DirItem, Error, FileType, RamFs, has_same_name};
use crate::index::{FrameAccess, FrameIndex, FramePtr, InodeIndex, RAMFS_ROOT_INODE_INDEX};
use crate::inode::IndexNode;
use core::ops::{Index, IndexMut};
use kernel_utils::{Mutex, info, trace};

static CURRENT_DIR: Mutex<InodeIndex> = Mutex::new(RAMFS_ROOT_INODE_INDEX);

impl RamFs {
    pub fn magic(&self) -> [u8; 8] {
        self.magic
    }
    pub fn fs_size(&self) -> u64 {
        self.fs_size
    }
    pub fn fs_start(&self) -> u64 {
        self as *const _ as u64
    }
    pub fn fs_end(&self) -> u64 {
        self.fs_start() + self.fs_size
    }
    pub fn fs_first_frame_addr(&self) -> u64 {
        self.first_frame_offset + self.fs_start()
    }
    pub fn total_frames(&self) -> u64 {
        (self.fs_end() - self.fs_first_frame_addr()) / (RAMFS_FRAME_SIZE_BYTE as u64)
    }
    pub(crate) fn resolve_path<'a>(&self, path: &'a str) -> Result<(InodeIndex, &'a str), Error> {
        match path.rsplit_once('/') {
            None => Ok((*CURRENT_DIR.lock(), path)),
            Some(("", name)) => Ok((RAMFS_ROOT_INODE_INDEX, name)),
            Some((parent_path, name)) => self.search_dir(parent_path).map(|s| (s, name)),
        }
    }
    pub fn dir_items(&self, inode: &IndexNode) -> Result<&[DirItem], Error> {
        if inode.file_type != FileType::Directory {
            return Err(Error::FileTypeIllegal);
        }
        match inode.page_1[0].to_option() {
            None => Ok(&[]),
            Some(frame_idx) => Ok(self[frame_idx]
                .as_dir_items(inode.size as usize / structs_size::RAMFS_DIR_ITEM_SIZE)),
        }
    }
    pub fn dir_items_from_idx(&self, idx: InodeIndex) -> Result<&[DirItem], Error> {
        let inode = &self[idx];
        if inode.file_type != FileType::Directory {
            return Err(Error::FileTypeIllegal);
        }
        match inode.page_1[0].to_option() {
            None => Ok(&[]),
            Some(frame_idx) => Ok(self[frame_idx]
                .as_dir_items(inode.size as usize / structs_size::RAMFS_DIR_ITEM_SIZE)),
        }
    }
    fn search_file_at_dir(
        &self,
        dir: InodeIndex,
        file_name: &str,
        file_type: FileType,
    ) -> Option<InodeIndex> {
        // dir type will be checked here
        self.dir_items(&self[dir])
            .ok()?
            .iter()
            .find(|item| has_same_name(&item.name, file_name) && item.file_type == file_type)
            .map(|item| item.index)
    }
    fn search_dir(&self, dir_path: &str) -> Result<InodeIndex, Error> {
        let mut temp = RAMFS_ROOT_INODE_INDEX;
        if !dir_path.starts_with(RAMFS_PATH_DELIMITER) {
            temp = *CURRENT_DIR.lock();
        }
        let path = dir_path.split(RAMFS_PATH_DELIMITER);
        for dir_name in path {
            match dir_name {
                "." => (),
                "" => return Err(Error::FilePathEmptyDir),
                ".." => {
                    if temp == RAMFS_ROOT_INODE_INDEX {
                        return Err(Error::NoSuchDirectory);
                    }
                    temp = self[temp].parent
                }
                _ => match self.search_file_at_dir(temp, dir_name, FileType::Directory) {
                    Some(item) => temp = item,
                    None => return Err(Error::NoSuchDirectory),
                },
            }
        }
        Ok(temp)
    }
    pub(crate) fn alloc_inode(&mut self) -> Result<InodeIndex, Error> {
        let idx = self
            .inode_bitmap
            .find_first_zero()
            .ok_or(Error::NoFreeInode)?;
        self.inode_bitmap.set(idx);
        Ok(InodeIndex::new_unchecked(idx))
    }
    pub(crate) fn free_inode(&mut self, idx: InodeIndex) -> Result<(), Error> {
        // free_inode_frame
        if idx == RAMFS_ROOT_INODE_INDEX {
            return Err(Error::TryToFreeRoot);
        }
        let inode = &self[idx];
        // TODO: support page_2
        for p in inode.page_1 {
            if let Some(p) = p.to_option() {
                self.free_frame(p);
            }
        }
        self.inode_bitmap.clear(*idx);
        Ok(())
    }
    pub fn new_inode(
        &mut self,
        name: &str,
        file_type: FileType,
        parent: InodeIndex,
    ) -> Result<InodeIndex, Error> {
        let idx = self.alloc_inode()?;
        self[idx].init(file_type, name, parent, idx)?;
        Ok(idx)
    }
    pub(crate) fn alloc_frame(&mut self) -> Result<FrameIndex, Error> {
        let idx = self
            .frame_bitmap
            .find_first_zero()
            .ok_or(Error::NoFreeFrame)?;
        self.frame_bitmap.set(idx);
        Ok(FrameIndex::new_unchecked(idx as u64))
    }
    pub(crate) fn alloc_zeroed_frame(&mut self) -> Result<FrameIndex, Error> {
        let frame = self.alloc_frame()?;
        self[frame].fill(0);
        Ok(frame)
    }
    pub(crate) fn alloc_ones_frame(&mut self) -> Result<FrameIndex, Error> {
        let frame = self.alloc_frame()?;
        self[frame].fill(u8::MAX);
        Ok(frame)
    }
    pub(crate) fn free_frame(&mut self, idx: FrameIndex) {
        self.frame_bitmap.clear(*idx as _);
    }
    fn index_of_inode(&self, inode: &IndexNode) -> InodeIndex {
        InodeIndex::new_unchecked(unsafe {
            (inode as *const IndexNode).offset_from(&self[RAMFS_ROOT_INODE_INDEX] as *const _)
        } as usize)
    }
    pub(crate) fn add_dir_entry(
        &mut self,
        parent_idx: InodeIndex,
        inode_idx: InodeIndex,
    ) -> Result<(), Error> {
        trace!("FS add_dir_entry");
        let frame_idx = self.get_or_init_inode_nth_frame(parent_idx, 0)?;
        trace!("fs_ptr: {:x?}", self as *const _);
        let parent = &mut self[parent_idx];
        trace!("parent_ptr: {:x?}", parent as *const _);
        trace!(
            "parent type: {}, name_bs: {:?}, size: {}",
            parent.file_type, parent.name, parent.size
        );

        let item_idx = parent.size as usize / structs_size::RAMFS_DIR_ITEM_SIZE;

        trace!("ParentDir size before add: {}", parent.size);
        parent.size += structs_size::RAMFS_DIR_ITEM_SIZE as u64;
        trace!("ParentDir size after  add: {}", parent.size);

        let name = self[inode_idx].name;
        let file_type = self[inode_idx].file_type;
        let item = self[frame_idx].nth_item_mut(item_idx);

        item.name.copy_from_slice(&name);
        item.file_type = file_type;
        item.index = inode_idx;

        trace!("{name}: {ty}", name = item.name(), ty = item.file_type);
        Ok(())
    }

    pub fn ls(&self) -> &[DirItem] {
        let dir = *CURRENT_DIR.lock();
        self.dir_items_from_idx(dir).unwrap_or(&[])
    }

    pub fn print_info(&self) {
        info!("RamFs:");
        info!("v_start: {}", self.fs_start());
        info!("size: {}", self.fs_size);
        info!("end: {}", self.fs_end());
        info!("first frame: {}", self.fs_first_frame_addr());
        info!("total frames: {}", self.total_frames());
    }

    pub unsafe fn as_raw_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.fs_start() as *const u8, self.fs_size as usize) }
    }

    pub fn get_inode_nth_frame(&self, idx: InodeIndex, nth: usize) -> FramePtr {
        let node = &self[idx];
        let Some(access) = FrameAccess::new(nth) else {
            return FramePtr::none();
        };
        match access {
            FrameAccess::Level1(i1) => node.page_1[i1],
            FrameAccess::Level2(i1, i2) => {
                let Some(f) = node.page_2[i1].to_option() else {
                    return FramePtr::none();
                };
                self[f].as_frame_ptrs()[i2]
            }
            FrameAccess::Level3(i1, i2, i3) => {
                let Some(f) = node.page_3[i1].to_option() else {
                    return FramePtr::none();
                };
                let f = self[f].as_frame_ptrs()[i2];
                let Some(f) = f.to_option() else {
                    return FramePtr::none();
                };
                self[f].as_frame_ptrs()[i3]
            }
        }
    }

    // TODO: add transaction
    pub fn get_or_init_inode_nth_frame(
        &mut self,
        idx: InodeIndex,
        nth: usize,
    ) -> Result<FrameIndex, Error> {
        let Some(access) = FrameAccess::new(nth) else {
            return Err(Error::NoSuchFrame);
        };

        match access {
            FrameAccess::Level1(i1) => {
                if self[idx].page_1[i1].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[idx].page_1[i1].emplace(frame);
                    info!("alloc frame at page1[{}], frame: {:?}", i1, frame);
                }
                Ok(self[idx].page_1[i1].unwrap())
            }
            FrameAccess::Level2(i1, i2) => {
                if self[idx].page_2[i1].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[idx].page_2[i1].emplace(frame);
                    info!("alloc frame at page2[{}], frame: {:?}", i1, frame);
                }
                let level1_frame_idx = self[idx].page_2[i1].unwrap();
                if self[level1_frame_idx].as_frame_ptrs()[i2].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[level1_frame_idx].as_frame_ptrs_mut()[i2].emplace(frame);
                    info!("alloc frame at page2[{}][{}], frame: {:?}", i1, i2, frame);
                }
                Ok(self[level1_frame_idx].as_frame_ptrs()[i2].unwrap())
            }
            FrameAccess::Level3(i1, i2, i3) => {
                if self[idx].page_3[i1].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[idx].page_3[i1].emplace(frame);
                    info!("alloc frame at page3[{}], frame: {:?}", i1, frame);
                }
                let level1_frame_idx = self[idx].page_3[i1].unwrap();
                if self[level1_frame_idx].as_frame_ptrs()[i2].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[level1_frame_idx].as_frame_ptrs_mut()[i2].emplace(frame);
                    info!("alloc frame at page3[{}][{}], frame: {:?}", i1, i2, frame);
                }
                let level2_frame_idx = self[level1_frame_idx].as_frame_ptrs()[i2].unwrap();
                if self[level2_frame_idx].as_frame_ptrs()[i2].is_none() {
                    let frame = self.alloc_ones_frame()?;
                    self[level2_frame_idx].as_frame_ptrs_mut()[i2].emplace(frame);
                    info!(
                        "alloc frame at page3[{}][{}][{}], frame: {:?}",
                        i1, i2, i3, frame
                    );
                }
                Ok(self[level2_frame_idx].as_frame_ptrs()[i3].unwrap())
            }
        }
    }
}

impl Index<InodeIndex> for RamFs {
    type Output = IndexNode;

    fn index(&self, index: InodeIndex) -> &Self::Output {
        &self.inodes[*index]
    }
}

impl IndexMut<InodeIndex> for RamFs {
    fn index_mut(&mut self, index: InodeIndex) -> &mut Self::Output {
        &mut self.inodes[*index]
    }
}

impl Index<FrameIndex> for RamFs {
    type Output = Frame;
    fn index(&self, index: FrameIndex) -> &Self::Output {
        unsafe { &*(self.fs_first_frame_addr() as *const Frame).add(*index as usize) }
    }
}
impl IndexMut<FrameIndex> for RamFs {
    fn index_mut(&mut self, index: FrameIndex) -> &mut Self::Output {
        unsafe { &mut *(self.fs_first_frame_addr() as *mut Frame).add(*index as usize) }
    }
}
