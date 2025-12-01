mod ramfs;
mod region;
mod vfs_impl;

use crate::constant::{RAMFS_FRAME_BITS, RAMFS_INODE_BITS, RAMFS_INODE_COUNT};
use crate::index::InodeIndex;
use crate::inode::IndexNode;
use core::ptr::NonNull;
use kernel_utils::{BitMap, string_enum};

pub struct RamFsMappedRegion {
    start: NonNull<u8>,
    size: usize,
}

// WARNING: The repr(C) is important, if not the compiler will reorder the fields
// It will cause ambiguous result
#[repr(C)]
pub struct RamFs {
    pub(super) magic: [u8; 8],
    pub(super) fs_size: u64,
    pub(super) inode_bitmap: BitMap<RAMFS_INODE_BITS>, // 0 means usable, 1 means unusable
    pub(super) frame_bitmap: BitMap<RAMFS_FRAME_BITS>, // 0 means usable, 1 means unusable
    pub(super) inodes: [IndexNode; RAMFS_INODE_COUNT],
    pub(super) first_frame_offset: u64,
    // padding
    // frames
}

string_enum! {
    #[derive(Debug, PartialEq, Eq)]
    pub enum Error {
        NoFreeInode,
        NoFreeFrame,
        FileNameIllegal,
        FileNameTooLong,
        FilePathEmptyDir,
        NoSuchDirectory,
        NoSuchFile,
        NotSupported,
        FileSystemError,
        FileTypeIllegal,
        TryToFreeRoot,
    }

    #[derive(Debug)]
    pub enum FileSystemError {
        BuildError,
        LoadError,
        MagicError,
        SizeError,
        AlignError,
        GlobalNotInit,
    }

    #[repr(u32)]
    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum FileType {
        File = 0,
        Directory,
        HardLink,
        SymLink,
    }
}

#[repr(C)]
pub struct DirItem {
    name: [u8; 32],
    file_type: FileType,
    index: InodeIndex,
}

impl DirItem {
    pub fn name(&self) -> &str {
        for i in 0..self.name.len() {
            if self.name[i] == 0 {
                return core::str::from_utf8(&self.name[..i]).unwrap_or("");
            }
        }
        ""
    }
    pub fn file_type(&self) -> FileType {
        self.file_type
    }
}

fn has_same_name(raw: &[u8; 32], name: &str) -> bool {
    let bs = name.as_bytes();
    if bs.len() >= 32 {
        return false;
    }
    &raw[..bs.len()] == bs && raw[bs.len()] == b'\0'
}

pub fn name_of(raw: &[u8; 32]) -> Option<&str> {
    for i in 0..raw.len() {
        if raw[i] == 0 {
            return core::str::from_utf8(&raw[..i]).ok();
        }
    }
    None
}
