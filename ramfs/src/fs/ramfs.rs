use crate::constant::{RAMFS_DIR_ITEM_SIZE_BYTE, RAMFS_FRAME_SIZE_BYTE, RAMFS_PATH_DELIMITER};
use crate::frame::Frame;
use crate::fs::{DirItem, Error, FileType, RamFs, has_same_name};
use crate::index::{FrameIndex, InodeIndex, RAMFS_ROOT_INODE_INDEX};
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
    // pub fn borrow_mut(&self) -> &mut Self {
    //     let ptr = self as *const RamFs;
    //     let mut c = NonNull::new(ptr as *mut _).unwrap();
    //     unsafe { c.as_mut() }
    // }
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
            Some(frame_idx) => {
                Ok(self[frame_idx].as_dir_items(inode.size as usize / RAMFS_DIR_ITEM_SIZE_BYTE))
            }
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
    pub(crate) fn alloc_frame(&mut self) -> Result<FrameIndex, Error> {
        let idx = self
            .frame_bitmap
            .find_first_zero()
            .ok_or(Error::NoFreeFrame)?;
        self.frame_bitmap.set(idx);
        Ok(FrameIndex::new_unchecked(idx as u64))
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

        let item_idx = parent.size as usize / RAMFS_DIR_ITEM_SIZE_BYTE;

        trace!("ParentDir size before add: {}", parent.size);
        parent.size += RAMFS_DIR_ITEM_SIZE_BYTE as u64;
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
        self.dir_items(&self[dir]).unwrap_or(&[])
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

    pub fn get_or_init_inode_nth_frame(
        &mut self,
        idx: InodeIndex,
        nth: usize,
    ) -> Result<FrameIndex, Error> {
        let node = &self[idx];
        let need_init = node.nth_frame(nth);
        if let Some(p) = need_init.to_option() {
            Ok(p)
        } else {
            let frame = self.alloc_frame()?;
            self[idx].nth_frame_mut(nth).emplace(frame);
            Ok(frame)
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
