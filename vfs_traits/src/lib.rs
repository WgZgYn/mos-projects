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
    /// Downcast support for filesystem-specific operations (e.g. link(2)).
    fn as_any(&self) -> &dyn core::any::Any;

    // fn to_stream_ops(self: Arc<Self>) -> VfsResult<Arc<dyn StreamOps>> { Err(VfsError::NotFile) }
    // fn to_file_ops(self: Arc<Self>) -> VfsResult<Arc<dyn FileOps>> { Err(VfsError::NotStream) }
    // fn to_dir_ops(self: Arc<Self>) -> VfsResult<Arc<dyn DirOps>> { Err(VfsError::NotDir) }

    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Err(VfsError::NotFile)
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
    /// truncate(2)/ftruncate(2): resize the file. Default: unsupported.
    fn truncate(&self, _len: u64) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// ioctl(2): device control. `arg` is a user-space pointer for the
    /// requests that copy data (TCGETS/TIOCGWINSZ). Default: ENOSYS.
    fn ioctl(&self, _cmd: u32, _arg: u64) -> VfsResult<usize> {
        Err(VfsError::NotSupported)
    }
}

pub trait FileOps: Send + Sync {
    fn read(&self, ctx: &IoContext, offset: usize, buf: &mut [u8]) -> VfsResult<usize>;
    fn write(&self, ctx: &IoContext, offset: usize, buf: &[u8]) -> VfsResult<usize>;
}

pub trait DirOps: Send + Sync {
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>>;
    fn create(&self, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;

    /// unlink(2)/rmdir(2): delete the child entry (and the inode) by name.
    /// Directory-ness / non-empty checks live in the syscall layer.
    fn remove(&self, _name: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// rename(2) within this directory.
    fn rename(&self, _old_name: &str, _new_name: &str) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
    /// link(2): insert a new entry in this directory pointing at an
    /// existing inode (no nlink accounting yet).
    fn link(&self, _name: &str, _node: &Arc<dyn Inode>) -> VfsResult<()> {
        Err(VfsError::NotSupported)
    }
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
