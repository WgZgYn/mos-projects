use crate::constant::{inode, RAMFS_ROOT_INODE_INDEX_USIZE};
use crate::fs::{FileType, RamFs};
use core::ops::Deref;

// this mod prevent construct with field
pub const RAMFS_ROOT_INODE_INDEX: InodeIndex = InodeIndex(RAMFS_ROOT_INODE_INDEX_USIZE);

/// NOT NULL
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(C)]
pub struct InodeIndex(usize);
impl Deref for InodeIndex {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl InodeIndex {
    pub fn new_unchecked(idx: usize) -> Self {
        Self(idx)
    }
}

/// NOT NULL
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FrameIndex(u64);

impl FrameIndex {
    pub fn new_unchecked(idx: u64) -> Self {
        FrameIndex(idx)
    }
}

impl Deref for FrameIndex {
    type Target = u64;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// NULLABLE
#[derive(Debug, Copy, Clone, PartialEq)]
#[repr(C)]
pub struct FramePtr(u64);

impl FramePtr {
    pub const fn none() -> Self {
        FramePtr(u64::MAX)
    }
    pub const fn is_none(&self) -> bool {
        self.0 == u64::MAX
    }
    pub const fn is_some(&self) -> bool {
        !self.is_none()
    }
    pub const fn unwrap(self) -> FrameIndex {
        if self.is_some() {
            FrameIndex(self.0)
        } else {
            panic!("unwarp none FramePtr")
        }
    }
    pub fn or_insert(&mut self, f: impl FnOnce() -> FrameIndex) -> FrameIndex {
        if self.is_none() {
            let idx = f();
            self.emplace(idx);
        }
        self.unwrap()
    }
    pub const fn emplace(&mut self, idx: FrameIndex) {
        self.0 = idx.0;
    }
    pub const fn to_option(self) -> Option<FrameIndex> {
        if self.is_some() {
            Some(FrameIndex(self.0))
        } else {
            None
        }
    }
}

pub enum FrameAccess {
    Level1(usize),
    Level2(usize, usize),
    Level3(usize, usize, usize),
}

impl FrameAccess {
    pub fn new(nth: usize) -> Option<Self> {
        if nth < inode::RAMFS_FRAME_BOUND_1 {
            Some(FrameAccess::Level1(nth))
        } else if nth < inode::RAMFS_FRAME_BOUND_2 {
            let remain = nth - inode::RAMFS_FRAME_BOUND_1;
            let index1 = remain / inode::RAMFS_FRAMES_PRE_PTR_2;
            let index2 = remain % inode::RAMFS_FRAMES_PRE_PTR_2;
            Some(FrameAccess::Level2(index1, index2))
        } else if nth < inode::RAMFS_FRAME_BOUND_3 {
            let remain = nth - inode::RAMFS_FRAME_BOUND_2;
            let index1 = remain / inode::RAMFS_FRAMES_PER_PTR_3;
            let remain = remain % inode::RAMFS_FRAMES_PER_PTR_3;
            let index2 = remain / inode::RAMFS_FRAMES_PRE_PTR_2;
            let index3 = remain % inode::RAMFS_FRAMES_PRE_PTR_2;
            Some(FrameAccess::Level3(index1, index2, index3))
        } else {
            None
        }
    }
}

pub struct DnodeIndex(InodeIndex);
impl Deref for DnodeIndex {
    type Target = InodeIndex;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DnodeIndex {
    pub fn new(fs: &RamFs, idx: InodeIndex) -> Option<Self> {
        if fs[idx].file_type == FileType::Directory {
            Some(DnodeIndex(idx))
        } else {
            None
        }
    }
}