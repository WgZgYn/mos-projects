use crate::DEV_DIR_INODE;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_arch::rand::sys_rand;
use vfs_common::{FileType, IoContext, VfsError, VfsResult};
use vfs_traits::*;

pub struct RandomInode;

lazy_static! {
    pub static ref RAND_INODE: Arc<RandomInode> = Arc::new(RandomInode);
}

impl FileOps for RandomInode {
    fn read(&self, _ctx: &IoContext, _offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        let mut i = 0;
        while i < buf.len() {
            let r = sys_rand().to_be_bytes();
            let n = (buf.len() - i).min(8);
            buf[i..i + n].copy_from_slice(&r[..n]);
            i += n;
        }
        Ok(buf.len())
    }

    fn write(&self, _ctx: &IoContext, _offset: usize, _buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }
}

impl Inode for RandomInode {
    fn file_type(&self) -> FileType {
        FileType::CharDevice
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
}
