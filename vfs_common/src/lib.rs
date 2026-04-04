#![no_std]

use bitflags::bitflags;
use num_enum::TryFromPrimitive;
use string_num::string_enum;

pub mod path;

pub type VfsResult<T> = Result<T, VfsError>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileType {
    Directory,

    Regular,
    BlockDevice,

    CharDevice,
    Fifo,
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
        const NONBLOCK = 1 << 6;
    }
}

// below is dynamic dispatch compatible
string_enum!(
    #[derive(Copy, Clone, Debug, TryFromPrimitive)]
    #[repr(usize)]
    pub enum VfsError {
        NotFound = 1,
        NotDir,
        NotFile,
        NotStream,
        NoSpace,
        // can't read / can't write
        PermissionDenied,
        IoError,
        AlreadyExists,
        FileNameIllegal,
        NotOpen,
        BadFileDesc,
        NotSupported,
        BrokenPipe,
        WouldBlock,
        Pendding,
        //
        InternelError,
        Unknown,
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

pub struct IoContext {
    pub nonblock: bool,
}

impl IoContext {
    pub const fn new(nonblock: bool) -> Self {
        Self { nonblock }
    }
}

fn vfs_error_to_usize(ret: VfsError) -> usize {
    let ret = ret as usize;
    assert_ne!(ret, 0);
    ret
}

fn usize_to_vfs_error(ret: usize) -> Option<VfsError> {
    VfsError::try_from_primitive(ret).ok()
}

pub fn vfs_result_to_isize(ret: VfsResult<usize>) -> isize {
    match ret {
        Ok(ret) => {
            let ret = ret as isize;
            assert!(!ret.is_negative());
            ret
        }
        Err(err) => {
            let ret = vfs_error_to_usize(err);
            -(ret as isize)
        }
    }
}

/// regard negative as error
pub fn isize_to_vfs_result(ret: isize) -> VfsResult<usize> {
    if !ret.is_negative() {
        Ok(ret as _)
    } else {
        Err(usize_to_vfs_error((-ret) as _).unwrap_or(VfsError::Unknown))
    }
}

mod tests {

    #[test]
    fn test_vfs_error_to_usize() {
        use crate::VfsError;
        let all = VfsError::all_variants();
        for i in 0..all.len() {
            assert_eq!(all[i] as usize, i + 1);
        }
    }
}
