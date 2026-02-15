use crate::{FileOps, VfsError, VfsResult};
use alloc::sync::Arc;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};

pub struct File {
    inode: Arc<dyn FileOps>,
    offset: AtomicUsize,
}

impl File {
    pub fn from_inode(inode: Arc<dyn FileOps>) -> Self {
        Self::new(inode, 0)
    }
    pub fn new(inode: Arc<dyn FileOps>, offset: usize) -> Self {
        Self {
            inode,
            offset: AtomicUsize::new(offset),
        }
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        let off = self.offset.load(Ordering::Relaxed);
        let n = self.inode.read_at(off, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        let off = self.offset.load(Ordering::Relaxed);
        let n = self.inode.write_at(off, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn seek(&self, offset: usize) {
        self.offset.store(offset, Ordering::SeqCst);
    }
    pub fn dup(self) -> DupFile {
        DupFile(Arc::new(self))
    }
}

#[derive(Clone)]
pub struct DupFile(Arc<File>);
impl Deref for DupFile {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ReadOnlyFile(File);
impl ReadOnlyFile {
    fn new(file: File) -> Self {
        Self(file)
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        let off = self.0.offset.load(Ordering::Relaxed);
        let n = self.0.inode.read_at(off, buf)?;
        self.0.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn write(&self, _buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }
    pub fn seek(&self, offset: usize) {
        self.0.offset.store(offset, Ordering::SeqCst);
    }
}
