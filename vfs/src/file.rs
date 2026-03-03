use crate::{DirEntry, FileHandler, Inode, OpenFlags, VfsError, VfsResult};
use alloc::sync::Arc;
use kernel_utils::Mutex;

pub struct File {
    pub inode: Arc<dyn Inode>,
    offset: u64,
    flags: OpenFlags,
}

impl File {
    pub fn inode(&self) -> &Arc<dyn Inode> {
        &self.inode
    }
    pub fn new(inode: Arc<dyn Inode>, offset: u64, flags: OpenFlags) -> Self {
        Self {
            inode,
            offset,
            flags,
        }
    }
    pub fn read(&mut self, buf: &mut [u8]) -> VfsResult<usize> {
        let n = self.inode.read_at(self.offset as _, buf)?;
        self.offset += n as u64;
        Ok(n)
    }
    pub fn write(&mut self, buf: &[u8]) -> VfsResult<usize> {
        let n = self.inode.write_at(self.offset as _, buf)?;
        self.offset += n as u64;
        Ok(n)
    }
    pub fn set_offset(&mut self, offset: u64) {
        self.offset = offset;
    }
    pub fn is_dir(&self) -> bool {
        self.flags.contains(OpenFlags::DIRECTORY)
    }
    fn read_dir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        if !self.is_dir() {
            Err(VfsError::NotDir)
        } else {
            self.inode.readdir(offset)
        }
    }
}

pub type FileRef = Arc<Mutex<File>>;

pub struct RegularFile {
    inner: Mutex<File>,
}

impl RegularFile {
    pub fn new(file: File) -> Self {
        Self {
            inner: Mutex::new(file),
        }
    }
}

impl FileHandler for RegularFile {
    fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        self.inner.lock().read(buf)
    }

    fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        self.inner.lock().write(buf)
    }

    fn read_dir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        self.inner.lock().read_dir(offset)
    }
}
