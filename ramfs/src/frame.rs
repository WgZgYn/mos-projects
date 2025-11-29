use core::ops::{Deref, DerefMut};
use crate::constant::{RAMFS_FRAME_SIZE_BYTE, RAMFS_MAX_DIR_ITEM_COUNT_PRE_FRAME};
use crate::fs::DirItem;

pub struct Frame([u8; RAMFS_FRAME_SIZE_BYTE]);
impl Deref for Frame {
    type Target = [u8; RAMFS_FRAME_SIZE_BYTE];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Frame {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
impl Frame {
    pub fn as_dir_items(&self, len: usize) -> &[DirItem] {
        unsafe { core::slice::from_raw_parts(&self.0 as *const _ as *const DirItem, len) }
    }
    pub fn as_dir_items_mut(&mut self, len: usize) -> &mut [DirItem] {
        unsafe { core::slice::from_raw_parts_mut(&mut self.0 as *mut _ as *mut DirItem, len) }
    }
    fn nth_item(&self, idx: usize) -> &DirItem {
        if idx >= RAMFS_MAX_DIR_ITEM_COUNT_PRE_FRAME {
            panic!("Index out of range");
        }
        &self.as_dir_items(RAMFS_FRAME_SIZE_BYTE / size_of::<DirItem>())[idx]
    }
    pub(crate) fn nth_item_mut(&mut self, idx: usize) -> &mut DirItem {
        if idx >= RAMFS_MAX_DIR_ITEM_COUNT_PRE_FRAME {
            panic!("Index out of range");
        }
        &mut self.as_dir_items_mut(RAMFS_FRAME_SIZE_BYTE / size_of::<DirItem>())[idx]
    }
}
