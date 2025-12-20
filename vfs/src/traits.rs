use crate::{DirEntry, FileOps, FileType, OpenFlags, VfsResult};
use alloc::boxed::Box;
use alloc::sync::Arc;

pub trait InodeOps {}


pub trait Inode: Send + Sync {
    fn id(&self) -> u64;
    fn file_type(&self) -> FileType;
    fn iop(&self) -> Arc<dyn InodeOps>;
    fn fop(&self, flags: OpenFlags) -> Arc<dyn FileOps>;
    fn lookup(&self, path: &str) -> Option<Arc<dyn Inode>>;
    fn readdir(&self) -> VfsResult<Box<dyn Iterator<Item = VfsResult<DirEntry>>>>;
}
