use crate::{DEV_DIR_DENTRY, DEV_DIR_INODE};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_arch::rand::sys_rand;
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use vfs_common::{FileType, IoContext, OpenFlags, VfsError, VfsResult};
use vfs_traits::*;

pub struct RandomInode;

lazy_static! {
    pub static ref RAND_INODE: Arc<RandomInode> = Arc::new(RandomInode);
    pub static ref RAND_DENTRY: Arc<Dentry> = DEV_DIR_DENTRY.lookup("rand").unwrap();
    pub static ref RAND_FILE: Arc<File> =
        Arc::new(File::from_dentry(RAND_DENTRY.clone(), 0, OpenFlags::READ).unwrap());
}

impl FileOps for RandomInode {
    fn read_at(&self, _offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        let mut i = 0;
        while i < buf.len() {
            let r = sys_rand().to_be_bytes();
            let n = (buf.len() - i).min(8);
            buf[i..i + n].copy_from_slice(&r[..n]);
            i += n;
        }
        Ok(buf.len())
    }

    fn write_at(&self, _offset: usize, _buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }
}

impl StreamOps for RandomInode {
    fn read(&self, _ctx: &IoContext, buf: &mut [u8]) -> VfsResult<usize> {
        let mut i = 0;
        while i < buf.len() {
            let r = sys_rand().to_be_bytes();
            let n = (buf.len() - i).min(8);
            buf[i..i + n].copy_from_slice(&r[..n]);
            i += n;
        }
        Ok(buf.len())
    }

    fn write(&self, _ctx: &IoContext, _buf: &[u8]) -> VfsResult<usize> {
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
    fn as_stream_ops(&self) -> VfsResult<&dyn StreamOps> {
        Ok(self)
    }
}
