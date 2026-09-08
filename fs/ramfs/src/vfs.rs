use crate::RamFsManager;
use crate::constant::{RAMFS_FRAME_SIZE_BYTE, structs_size};
use crate::fs::{DirItem, FileType};
use crate::index::{InodeIndex, RAMFS_ROOT_INODE_INDEX};
use alloc::borrow::ToOwned;
use alloc::sync::Arc;
use log::{info, trace};
use vfs_common::{IoContext, MountFlags, VfsError, VfsResult};
use vfs_traits::{DirEntry, DirOps, FileOps, Inode, SuperBlock, VFS};

impl VFS for RamFsManager<'static> {
    fn name(&self) -> Option<&str> {
        Some("ramfsWZY")
    }
    fn mount(
        &self,
        _dev: Option<&str>,
        _flags: MountFlags,
    ) -> Result<Arc<dyn SuperBlock>, VfsError> {
        todo!()
    }
}

pub struct FsNode {
    fs: Arc<RamFsManager<'static>>,
    idx: InodeIndex,
}

impl FileOps for FsNode {
    fn read(&self, _ctx: &IoContext, offset: usize, buf: &mut [u8]) -> Result<usize, VfsError> {
        let fs = self.fs.read();
        let index = self.idx;
        let inode = &fs[index];
        let mut cursor = offset;
        if cursor as u64 >= inode.size {
            return Ok(0);
        }
        let total_available = (inode.size as usize - cursor).min(buf.len());
        drop(fs);
        let mut read_total = 0;
        while read_total < total_available {
            let frame_idx = cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = cursor % RAMFS_FRAME_SIZE_BYTE;
            let fs = self.fs.read();
            let frame_idx = fs
                .get_inode_nth_frame(index, frame_idx)
                .to_option()
                .ok_or(VfsError::NoSpace)?;
            let frame = &fs[frame_idx];
            let frame_remain = RAMFS_FRAME_SIZE_BYTE - offset;
            let remaining = total_available - read_total;
            let n = frame_remain.min(remaining);
            let data = frame[offset..offset + n].to_vec();
            drop(fs);
            buf[read_total..read_total + n].copy_from_slice(&data);
            cursor += n;
            read_total += n;
        }
        Ok(read_total)
    }

    fn write(&self, _ctx: &IoContext, offset: usize, mut buf: &[u8]) -> Result<usize, VfsError> {
        let index = self.idx;
        let mut cursor = offset;
        let mut total_written = 0;
        while !buf.is_empty() {
            let frame_nth = cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = cursor % RAMFS_FRAME_SIZE_BYTE;
            let writable = (RAMFS_FRAME_SIZE_BYTE - offset).min(buf.len());
            let data = buf[..writable].to_vec();
            let mut fs = self.fs.write();
            let frame_idx = fs
                .get_or_init_inode_nth_frame(index, frame_nth)
                .map_err(|_| VfsError::NoSpace)?;
            let frame = &mut fs[frame_idx];
            frame[offset..offset + writable].copy_from_slice(&data);
            let inode = &mut fs[index];
            cursor += writable;
            total_written += writable;
            if cursor > inode.size as usize {
                inode.size = cursor as u64;
            }
            buf = &buf[writable..];
        }
        Ok(total_written)
    }
}

impl DirOps for FsNode {
    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError> {
        self.fs
            .read()
            .dir_items_from_idx(self.idx)
            .map_err(|_| VfsError::NotDir)?
            .iter()
            .find(|i| i.has_same_name(name))
            .map(|i| {
                if let Some(weak) = self.fs.inodes.lock().get(&i.index()).cloned()
                    && let Some(inode) = weak.upgrade()
                {
                    inode
                } else {
                    let inode = Arc::new(FsNode {
                        fs: self.fs.clone(),
                        idx: i.index(),
                    }) as Arc<dyn Inode>;
                    self.fs
                        .inodes
                        .lock()
                        .insert(i.index(), Arc::downgrade(&inode));
                    inode
                }
            })
            .ok_or(VfsError::NotFound)
    }

    fn create(&self, name: &str, file_type: vfs_common::FileType) -> VfsResult<Arc<dyn Inode>> {
        let parent_inode_idx = self.idx;
        let file_type = match file_type {
            vfs_common::FileType::Regular => FileType::File,
            vfs_common::FileType::Directory => FileType::Directory,
            vfs_common::FileType::Fifo => FileType::Fifo,
            vfs_common::FileType::SymLink => FileType::SymLink,
            _ => return Err(VfsError::NotSupported),
        };
        info!("[RAMFS] create file {:?}", file_type);

        let mut fs = self.fs.write();

        let inode_idx = fs
            .new_inode(name, file_type, parent_inode_idx)
            .map_err(|_| VfsError::NoSpace)?;

        trace!("created inode {:?}", inode_idx);
        trace!(
            "add dir entry {:?} from parent {:?}",
            inode_idx, parent_inode_idx
        );
        fs.add_dir_entry(parent_inode_idx, inode_idx, name)
            .map_err(|_| VfsError::NoSpace)?;

        Ok(Arc::new(FsNode {
            fs: self.fs.clone(),
            idx: inode_idx,
        }))
    }

    fn remove(&self, name: &str) -> VfsResult<()> {
        let child_idx = self
            .fs
            .read()
            .find_child_by_name(self.idx, name)
            .ok_or(VfsError::NotFound)?;
        let mut fs = self.fs.write();
        fs.remove_dir_entry(self.idx, child_idx)
            .map_err(|_| VfsError::NotFound)?;
        fs.free_inode(child_idx).map_err(|_| VfsError::IoError)?;
        // drop cached Arc so the frames we just freed are not reused
        self.fs.inodes.lock().remove(&child_idx);
        Ok(())
    }

    fn rename(&self, old_name: &str, new_name: &str) -> VfsResult<()> {
        if old_name == new_name {
            return Ok(());
        }
        let child_idx = self
            .fs
            .read()
            .find_child_by_name(self.idx, old_name)
            .ok_or(VfsError::NotFound)?;
        if self
            .fs
            .read()
            .find_child_by_name(self.idx, new_name)
            .is_some()
        {
            return Err(VfsError::AlreadyExists);
        }
        self.fs
            .write()
            .rename_inode(self.idx, child_idx, new_name)
            .map_err(|_| VfsError::IoError)?;
        Ok(())
    }

    fn link(&self, name: &str, node: &Arc<dyn Inode>) -> VfsResult<()> {
        let Some(src) = node.as_any().downcast_ref::<FsNode>() else {
            return Err(VfsError::NotSupported);
        };
        if self
            .fs
            .read()
            .find_child_by_name(self.idx, name)
            .is_some()
        {
            return Err(VfsError::AlreadyExists);
        }
        self.fs
            .write()
            .add_dir_entry(self.idx, src.idx, name)
            .map_err(|_| VfsError::NoSpace)?;
        Ok(())
    }

    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        let node = Arc::new(FsNode {
            fs: self.fs.clone(),
            idx: self.idx,
        });

        trace!("dir size: {}", node.file_size());

        let bytes_offset = offset * structs_size::RAMFS_DIR_ITEM_SIZE;

        let mut item = [0; structs_size::RAMFS_DIR_ITEM_SIZE];
        let n = node.read(&IoContext::default(), bytes_offset, &mut item)?;
        if n == 0 {
            return Ok(None);
        }
        if n == structs_size::RAMFS_DIR_ITEM_SIZE {
            unsafe {
                let item: DirItem = core::mem::transmute(item);
                trace!("dir item: {:?}", item.name());

                let ft = match item.file_type() {
                    FileType::File => vfs_common::FileType::Regular,
                    FileType::Directory => vfs_common::FileType::Directory,
                    FileType::Fifo => vfs_common::FileType::Fifo,
                    FileType::SymLink => vfs_common::FileType::SymLink,
                    _ => return Err(VfsError::NotSupported),
                };

                return Ok(Some(DirEntry::new(
                    item.name().to_owned(),
                    ft,
                    Arc::new(FsNode {
                        fs: self.fs.clone(),
                        idx: item.index(),
                    }),
                )));
            };
        }
        Err(VfsError::NotDir)
    }
}

impl Inode for FsNode {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn meta_data(&self) -> &[u8] {
        &[]
    }

    fn file_size(&self) -> u64 {
        let fs = self.fs.read();
        let index = self.idx;
        let inode = &fs[index];
        inode.size
    }
    fn file_type(&self) -> vfs_common::FileType {
        let ft = self.fs.read()[self.idx].file_type;
        match ft {
            FileType::File => vfs_common::FileType::Regular,
            FileType::Directory => vfs_common::FileType::Directory,
            FileType::Fifo => vfs_common::FileType::Fifo,
            FileType::SymLink => vfs_common::FileType::SymLink,
            _ => unimplemented!(),
        }
    }
    fn truncate(&self, len: u64) -> VfsResult<()> {
        let old_size = { self.fs.read()[self.idx].size };
        if len > old_size {
            // extend with zeros via the normal write path
            let zeros = [0u8; RAMFS_FRAME_SIZE_BYTE];
            let mut off = old_size as usize;
            let mut rem = (len - old_size) as usize;
            while rem > 0 {
                let n = rem.min(RAMFS_FRAME_SIZE_BYTE);
                self.write(&IoContext::default(), off, &zeros[..n])?;
                off += n;
                rem -= n;
            }
            return Ok(());
        }
        self.fs
            .write()
            .truncate_shrink(self.idx, len)
            .map_err(|_| VfsError::IoError)
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        let parent = self.fs.read()[self.idx].parent;
        if parent == self.idx {
            return Ok(None);
        }
        Ok(Some(Arc::new(FsNode {
            fs: self.fs.clone(),
            idx: parent,
        })))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
    fn as_dir_ops(&self) -> VfsResult<&dyn DirOps> {
        let inode = &self.fs.read()[self.idx];
        if inode.file_type == FileType::Directory {
            Ok(self)
        } else {
            Err(VfsError::NotDir)
        }
    }
}

pub struct RamFsSuperBlock {
    block: Arc<RamFsManager<'static>>,
}

impl RamFsSuperBlock {
    pub fn new(block: RamFsManager<'static>) -> RamFsSuperBlock {
        RamFsSuperBlock {
            block: Arc::new(block),
        }
    }
}

impl SuperBlock for RamFsSuperBlock {
    fn root(&self) -> Arc<dyn Inode> {
        Arc::new(FsNode {
            fs: self.block.clone(),
            idx: RAMFS_ROOT_INODE_INDEX,
        })
    }
}
