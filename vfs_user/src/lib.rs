#![no_std]

use bitflags::bitflags;
use string_num::string_enum;

pub mod path;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileType {
    Regular,
    Directory,
    CharDevice(u32),
    BlockDevice(u32),
    Socket,
    Fifo,
    Link,
}

bitflags! {
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct OpenFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const READ_WRITE = OpenFlags::READ.bits() | OpenFlags::WRITE.bits();
        const CREATE = 1 << 2;
        const APPEND = 1 << 3;
        const TRUNC = 1 << 4;
        const DIRECTORY = 1 << 5;
    }
}

// below is dynamic dispatch compatible
string_enum!(
    #[derive(Debug)]
    pub enum VfsError {
        NotFound,
        NotDir,
        NotFile,
        NoSpace,
        PermissionDenied,
        IoError,
        AlreadyExists,
        FileNameIllegal,
        NotOpen,
        BadFileDesc,
    }
);

bitflags! {
    pub struct MountFlags: u32 {

    }
}

#[derive(Clone)]
pub struct UserDirEntry {
    pub name: [u8; 32],
    pub file_type: FileType,
    pub size: u64,
}

impl UserDirEntry {
    pub fn new(name: [u8; 32], file_type: FileType, size: u64) -> Self {
        Self {
            name,
            file_type,
            size,
        }
    }
    pub fn name(&self) -> Option<&str> {
        let mut i = 0;
        while i < self.name.len() {
            if self.name[i] == 0 {
                return core::str::from_utf8(&self.name[..i]).ok();
            }
            i += 1;
        }
        None
    }
}
