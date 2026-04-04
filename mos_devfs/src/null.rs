use crate::{DEV_DIR_DENTRY, DEV_DIR_INODE};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use vfs_common::{FileType, IoContext, OpenFlags, VfsResult};
use vfs_traits::*;

pub struct NullInode;

lazy_static! {
    pub static ref NULL_INODE: Arc<NullInode> = Arc::new(NullInode);
    pub static ref NULL_DENTRY: Arc<Dentry> = DEV_DIR_DENTRY.lookup("null").unwrap();
    pub static ref NULL_FILE: Arc<File> =
        Arc::new(File::from_dentry(NULL_DENTRY.clone(), 0, OpenFlags::READ).unwrap());
}

impl FileOps for NullInode {
    fn read_at(&self, _offset: usize, _buf: &mut [u8]) -> VfsResult<usize> {
        Ok(0)
    }

    fn write_at(&self, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        Ok(buf.len())
    }
}

impl StreamOps for NullInode {
    fn read(&self, _ctx: &IoContext, _buf: &mut [u8]) -> VfsResult<usize> {
        Ok(0)
    }

    fn write(&self, _ctx: &IoContext, buf: &[u8]) -> VfsResult<usize> {
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
