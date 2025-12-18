use crate::{DirEntry, FileOps, FileType, VfsResult};
use alloc::boxed::Box;
use alloc::sync::Arc;
use kernel_utils::bitflags;

pub trait InodeOps {}
bitflags! {
    pub struct OpenFlags: u32 {
        const READ = 1;
        const WRITE = 2;
        const READ_WRITE = OpenFlags::READ.bits() | OpenFlags::WRITE.bits();
        const CREATE = 4;
        const APPEND = 8;
        const TRUNC = 16;
    }
}

pub trait Inode: Send + Sync {
    fn id(&self) -> u64;
    fn file_type(&self) -> FileType;
    fn iop(&self) -> Arc<dyn InodeOps>;
    fn fop(&self, flags: OpenFlags) -> Arc<dyn FileOps>;
    fn lookup(&self, path: &str) -> Option<Arc<dyn Inode>>;
    fn readdir(&self) -> VfsResult<Box<dyn Iterator<Item = VfsResult<DirEntry>>>>;
}
