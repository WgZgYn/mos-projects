use crate::RamFsManager;
use crate::constant::{RAMFS_FRAME_SIZE_BYTE, structs_size};
use crate::fs::{DirItem, FileType};
use crate::index::{InodeIndex, RAMFS_ROOT_INODE_INDEX};
use alloc::borrow::ToOwned;
use alloc::sync::Arc;
use vfs::{DirEntry, DirInode, File, FileOps, VfsError, Inode, MountFlags, SuperBlock, VFS};

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
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> Result<usize, VfsError> {
        let fs = self.fs.lock();
        let index = self.idx;
        let inode = &fs[index];
        let mut cursor = offset;
        if cursor as u64 >= inode.size {
            return Ok(0);
        }
        let total_available = (inode.size as usize - cursor).min(buf.len());
        let mut read_total = 0;
        while read_total < total_available {
            let frame_idx = cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = cursor % RAMFS_FRAME_SIZE_BYTE;
            let frame_idx = fs
                .get_inode_nth_frame(index, frame_idx)
                .to_option()
                .ok_or(VfsError::NoSpace)?;
            let frame = &fs[frame_idx];
            let frame_remain = RAMFS_FRAME_SIZE_BYTE - offset;
            let remaining = total_available - read_total;
            let n = frame_remain.min(remaining);
            buf[read_total..read_total + n].copy_from_slice(&frame[offset..offset + n]);
            cursor += n;
            read_total += n;
        }
        Ok(read_total)
    }

    fn write_at(&self, offset: usize, mut buf: &[u8]) -> Result<usize, VfsError> {
        let mut fs = self.fs.lock();
        let index = self.idx;
        let mut cursor = offset;
        let mut total_written = 0;
        while !buf.is_empty() {
            let frame_nth = cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = cursor % RAMFS_FRAME_SIZE_BYTE;
            let frame_idx = fs
                .get_or_init_inode_nth_frame(index, frame_nth)
                .map_err(|_| VfsError::NoSpace)?;
            let frame = &mut fs[frame_idx];
            let writable = (RAMFS_FRAME_SIZE_BYTE - offset).min(buf.len());
            frame[offset..offset + writable].copy_from_slice(&buf[..writable]);
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

impl DirInode for FsNode {
    fn readdir(&self, offset: usize) -> Result<Option<DirEntry>, VfsError> {
        let fs = self.fs.lock();
        let node = &fs[self.idx];
        if node.file_type != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        drop(fs);
        let node = Arc::new(FsNode {
            fs: self.fs.clone(),
            idx: self.idx,
        });
        let file = File::new(node.clone());
        file.seek(offset * structs_size::RAMFS_DIR_ITEM_SIZE);
        let mut item = [0; structs_size::RAMFS_DIR_ITEM_SIZE];
        let n = file.read(&mut item)?;
        if n == 0 {
            return Ok(None);
        }
        if n == structs_size::RAMFS_DIR_ITEM_SIZE {
            unsafe {
                let item: DirItem = core::mem::transmute(item);
                return Ok(Some(DirEntry::new(
                    item.name().to_owned(),
                    if item.file_type() == FileType::Directory {
                        vfs::FileType::Directory
                    } else {
                        vfs::FileType::Regular
                    },
                    node,
                )));
            };
        }
        Err(VfsError::NotDir)
    }
}

impl Inode for FsNode {
    fn meta_data(&self) -> &[u8] {
        todo!()
    }

    fn size(&self) -> u64 {
        self.fs.lock()[self.idx].size
    }

    fn is_dir(&self) -> bool {
        self.fs.lock()[self.idx].file_type == FileType::Directory
    }

    fn parent(&self) -> Option<Arc<dyn Inode>> {
        Some(Arc::new(FsNode {
            fs: self.fs.clone(),
            idx: self.fs.lock()[self.idx].parent,
        }))
    }

    fn lookup(&self, name: &str) -> Result<Arc<dyn Inode>, VfsError> {
        self.fs
            .lock()
            .dir_items_from_idx(self.idx)
            .map_err(|_| VfsError::NotDir)?
            .iter()
            .find(|i| i.has_same_name(name))
            .map(|i| {
                Arc::new(FsNode {
                    fs: self.fs.clone(),
                    idx: i.index(),
                }) as Arc<dyn Inode>
            })
            .ok_or(VfsError::NotFound)
    }
    fn create(&self, name: &str, is_dir: bool) {}

    fn file_type(&self) -> vfs::FileType {
        if self.fs.lock()[self.idx].file_type == FileType::Directory {
            vfs::FileType::Directory
        } else {
            vfs::FileType::Regular
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
