use crate::fs::{Error, FileType};
use crate::fs_manager::RamFsManager;
use crate::index::InodeIndex;

mod vfs_impl;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FileDescriptor(InodeIndex);

#[derive(Debug)]
pub struct File {
    fd: InodeIndex,
    cursor: usize,
}

impl File {
    pub fn from_fd(fd: InodeIndex) -> File {
        File { fd, cursor: 0 }
    }

    // if the return str type is &str, it will hold immutable ref,
    // it will cause UB, if change the name at the same
    // only when rc or other method to make sure it will not be borrowed as mutable
    // currently easily impl it with String
    pub fn file_name(&self) -> Result<[u8; 32], Error> {
        Ok(RamFsManager::global()
            .map_err(|_| Error::FileSystemError)?
            .lock()[self.fd]
            .name)
    }
    pub fn file_type(&self) -> Result<FileType, Error> {
        Ok(RamFsManager::global()
            .map_err(|_| Error::FileSystemError)?
            .lock()[self.fd]
            .file_type)
    }
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
    }
    pub fn file_bytes(&self) -> u64 {
        let Ok(fs) = RamFsManager::global() else {
            return 0;
        };
        fs.lock()[self.fd].size
    }
}
