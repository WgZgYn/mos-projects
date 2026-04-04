use crate::{DEV_DIR_DENTRY, DEV_DIR_INODE};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_vfs::dentry::Dentry;
use vfs_common::{FileType, IoContext, VfsResult};
use vfs_traits::*;

pub struct ZeroInode;
lazy_static! {
    pub static ref ZERO_INODE: Arc<ZeroInode> = Arc::new(ZeroInode);
    pub static ref ZERO_DENTRY: Arc<Dentry> = DEV_DIR_DENTRY.lookup("zero").unwrap();
}

impl FileOps for ZeroInode {
    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        Ok(buf.len())
    }
}

impl StreamOps for ZeroInode {
    fn read(&self, _ctx: &IoContext, buf: &mut [u8]) -> VfsResult<usize> {
        buf.fill(0);
        Ok(buf.len())
    }

    fn write(&self, _ctx: &IoContext, buf: &[u8]) -> VfsResult<usize> {
        Ok(buf.len())
    }
}

impl Inode for ZeroInode {
    fn file_type(&self) -> FileType {
        FileType::Regular
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
    fn as_stream_ops(&self) -> VfsResult<&dyn StreamOps> {
        Ok(self)
    }
}
