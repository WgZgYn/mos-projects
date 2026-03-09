use crate::{DirEntry, Inode, OpenFlags, VfsError, VfsResult};
use alloc::sync::Arc;
use kernel_utils::Mutex;

pub struct File {
    inode: Arc<dyn Inode>,
    offset: Mutex<u64>,
    flags: OpenFlags,
}

impl File {
    pub fn into_inode(self) -> Arc<dyn Inode> {
        self.inode
    }
    pub fn inode(&self) -> &Arc<dyn Inode> {
        &self.inode
    }
    pub fn flags(&self) -> OpenFlags {
        self.flags
    }
    pub fn new(inode: Arc<dyn Inode>, offset: u64, flags: OpenFlags) -> Self {
        Self {
            inode,
            offset: Mutex::new(offset),
            flags,
        }
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        if !self.flags.contains(OpenFlags::READ) {
            return Err(VfsError::PermissionDenied);
        }
        let mut offset = self.offset.lock();
        let n = self.inode.read_at(*offset as _, buf)?;
        *offset += n as u64;
        Ok(n)
    }
    pub fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        if !self.flags.contains(OpenFlags::WRITE) {
            return Err(VfsError::PermissionDenied);
        }
        let mut offset = self.offset.lock();
        let n = self.inode.write_at(*offset as _, buf)?;
        *offset += n as u64;
        Ok(n)
    }
    pub fn set_offset(&mut self, offset: u64) {
        *self.offset.lock() = offset;
    }
    pub fn is_dir(&self) -> bool {
        self.flags.contains(OpenFlags::DIRECTORY)
    }
    pub fn read_dir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        if !self.is_dir() {
            Err(VfsError::NotDir)
        } else {
            if self.flags.contains(OpenFlags::DIRECTORY) {
                self.inode.readdir(offset)
            } else {
                Err(VfsError::PermissionDenied)
            }
        }
    }
}

