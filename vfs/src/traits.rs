use crate::{DirEntry, VfsResult};
use alloc::sync::Arc;
use vfs_user::{FileType, MountFlags};

pub trait VFS {
    fn name(&self) -> Option<&str>;
    fn mount(&self, dev: Option<&str>, flags: MountFlags) -> VfsResult<Arc<dyn SuperBlock>>;
}

pub trait SuperBlock {
    fn root(&self) -> Arc<dyn Inode>;
}

pub trait Inode: FileOps + InodeOps {
    fn meta_data(&self) -> &[u8];
    fn file_size(&self) -> u64;
    fn file_type(&self) -> FileType;
}

pub trait FileOps: Send + Sync {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize>;
}

pub trait InodeOps: Send + Sync {
    fn parent(&self) -> Option<Arc<dyn Inode>>;
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>>;
    fn create(self: Arc<Self>, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;
}
