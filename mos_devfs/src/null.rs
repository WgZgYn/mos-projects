use crate::DEV_DIR_INODE;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use vfs_common::{FileType, IoContext, VfsResult};
use vfs_traits::*;

pub struct NullInode;

lazy_static! {
    pub static ref NULL_INODE: Arc<NullInode> = Arc::new(NullInode);
}

impl FileOps for NullInode {
    fn read(&self, _ctx: &IoContext, _offset: usize, _buf: &mut [u8]) -> VfsResult<usize> {
        Ok(0)
    }

    fn write(&self, _ctx: &IoContext, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        Ok(buf.len())
    }
}

impl Inode for NullInode {
    fn file_type(&self) -> FileType {
        FileType::Regular
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
}
