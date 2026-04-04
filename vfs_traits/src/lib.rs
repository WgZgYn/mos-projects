#![no_std]
extern crate alloc;

use alloc::string::String;
use alloc::sync::Arc;
use vfs_common::{FileType, IoContext, MountFlags, OpenFlags, VfsError, VfsResult};

pub trait VFS {
    fn name(&self) -> Option<&str>;
    fn mount(&self, dev: Option<&str>, flags: MountFlags) -> VfsResult<Arc<dyn SuperBlock>>;
}

pub trait SuperBlock {
    fn root(&self) -> Arc<dyn Inode>;
}

pub trait Inode: Send + Sync {
    fn meta_data(&self) -> &[u8] {
        &[]
    }
    fn file_size(&self) -> u64 {
        0
    }
    fn file_type(&self) -> FileType;
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Err(VfsError::NotSupported)
    }

    // fn to_stream_ops(self: Arc<Self>) -> VfsResult<Arc<dyn StreamOps>> { Err(VfsError::NotFile) }
    // fn to_file_ops(self: Arc<Self>) -> VfsResult<Arc<dyn FileOps>> { Err(VfsError::NotStream) }
    // fn to_dir_ops(self: Arc<Self>) -> VfsResult<Arc<dyn DirOps>> { Err(VfsError::NotDir) }

    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Err(VfsError::NotFile)
    }
    fn as_stream_ops(&self) -> VfsResult<&dyn StreamOps> {
        Err(VfsError::NotStream)
    }
    fn as_dir_ops(&self) -> VfsResult<&dyn DirOps> {
        Err(VfsError::NotDir)
    }

    fn on_open(&self, _flags: OpenFlags) -> VfsResult<()> {
        Ok(())
    }
    fn on_close(&self, _flags: OpenFlags) -> VfsResult<()> {
        Ok(())
    }
}

pub trait StreamOps: Send + Sync {
    fn read(&self, ctx: &IoContext, buf: &mut [u8]) -> VfsResult<usize>;
    fn write(&self, ctx: &IoContext, buf: &[u8]) -> VfsResult<usize>;
}

pub trait FileOps: Send + Sync {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize>;
}

pub trait DirOps: Send + Sync {
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>>;
    fn create(&self, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;
}

#[derive(Clone)]
pub struct DirEntry {
    name: String,
    ft: FileType,
    node: Arc<dyn Inode>,
}

impl DirEntry {
    pub fn new(name: String, ft: FileType, node: Arc<dyn Inode>) -> Self {
        DirEntry { name, ft, node }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn file_type(&self) -> FileType {
        self.ft
    }
    pub fn node(&self) -> Arc<dyn Inode> {
        self.node.clone()
    }
}
