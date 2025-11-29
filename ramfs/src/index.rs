use core::ops::Deref;
use crate::constant::RAMFS_ROOT_INODE_INDEX_USIZE;

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
