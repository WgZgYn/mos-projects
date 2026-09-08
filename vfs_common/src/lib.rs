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
    SymLink,
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
        NotEmpty,
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
    pub const fn default() -> Self {
        Self { nonblock: false }
    }
    pub const fn new(nonblock: bool) -> Self {
        Self { nonblock }
    }
}

/// Map a VFS error to the Linux x86_64 errno, so negative syscall returns
/// are meaningful to musl/glibc callers.
pub fn vfs_error_to_errno(ret: VfsError) -> i64 {
    match ret {
        VfsError::NotFound => 2,           // ENOENT
        VfsError::NotDir => 20,            // ENOTDIR
        VfsError::NotFile => 21,           // EISDIR
        VfsError::NotStream => 29,         // ESPIPE
        VfsError::NoSpace => 28,           // ENOSPC
        VfsError::PermissionDenied => 13,  // EACCES
        VfsError::IoError => 5,            // EIO
        VfsError::AlreadyExists => 17,     // EEXIST
        VfsError::FileNameIllegal => 22,   // EINVAL
        VfsError::NotOpen => 9,            // EBADF
        VfsError::BadFileDesc => 9,        // EBADF
        VfsError::NotSupported => 38,      // ENOSYS
        VfsError::BrokenPipe => 32,        // EPIPE
        VfsError::WouldBlock => 11,        // EAGAIN
        VfsError::NotEmpty => 39,          // ENOTEMPTY
        VfsError::Pendding => 11,          // EAGAIN
        VfsError::InternelError => 5,      // EIO
        VfsError::Unknown => 5,            // EIO
    }
}

fn errno_to_vfs_error(errno: i64) -> Option<VfsError> {
    Some(match errno {
        2 => VfsError::NotFound,
        20 => VfsError::NotDir,
        21 => VfsError::NotFile,
        29 => VfsError::NotStream,
        28 => VfsError::NoSpace,
        13 => VfsError::PermissionDenied,
        5 => VfsError::IoError,
        17 => VfsError::AlreadyExists,
        22 => VfsError::FileNameIllegal,
        9 => VfsError::BadFileDesc,
        38 => VfsError::NotSupported,
        32 => VfsError::BrokenPipe,
        11 => VfsError::WouldBlock,
        39 => VfsError::NotEmpty,
        _ => return None,
    })
}

pub fn vfs_result_to_isize(ret: VfsResult<usize>) -> isize {
    match ret {
        Ok(ret) => {
            let ret = ret as isize;
            assert!(!ret.is_negative());
            ret
        }
        Err(err) => -vfs_error_to_errno(err) as isize,
    }
}

/// regard negative as error
pub fn isize_to_vfs_result(ret: isize) -> VfsResult<usize> {
    if !ret.is_negative() {
        Ok(ret as _)
    } else {
        Err(errno_to_vfs_error(-(ret as i64)).unwrap_or(VfsError::Unknown))
    }
}

mod tests {

    #[test]
    fn test_vfs_errno_roundtrip() {
        use crate::{VfsError, vfs_error_to_errno};
        let all = VfsError::all_variants();
        for err in all {
            let errno = vfs_error_to_errno(err);
            assert!(errno > 0 && errno < 4096, "errno out of range: {}", errno);
        }
    }
}
