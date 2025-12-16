pub const RAMFS_MEMORY_ADDRESS_START: u64 = 0xFFFF_8000_0200_0000;
pub const RAMFS_MEMORY_SIZE: u64 = 4 * 1024 * 1024;
pub const RAMFS_MEMORY_ADDRESS_END: u64 = RAMFS_MEMORY_ADDRESS_START + RAMFS_MEMORY_SIZE;

pub const RAMFS_MAGIC: [u8; 8] = *b"ramfsWZY";
pub const RAMFS_FRAME_SIZE_BYTE: usize = 4096; // 4KiB
pub const RAMFS_INODE_COUNT: usize = 512;
pub const RAMFS_INODE_BITS: usize = RAMFS_INODE_COUNT / 32;
pub const RAMFS_MAX_SIZE_BYTE: usize = 1024 * 1024 * 1024 * 4; // 4GiB
pub const RAMFS_FRAME_MAX_COUNT: usize = RAMFS_MAX_SIZE_BYTE / RAMFS_FRAME_SIZE_BYTE;
pub const RAMFS_FRAME_BITS: usize = RAMFS_FRAME_MAX_COUNT / 32;

pub const RAMFS_FILE_MAX_SIZE_BYTE: usize = 8 * RAMFS_FRAME_SIZE_BYTE
    + RAMFS_FRAME_SIZE_BYTE / 8 * RAMFS_FRAME_SIZE_BYTE
    + RAMFS_FRAME_SIZE_BYTE * RAMFS_FRAME_SIZE_BYTE / 8 * RAMFS_FRAME_SIZE_BYTE;

pub mod structs_size {
    use crate::fs::{DirItem, RamFs};
    use crate::index::{FrameIndex, InodeIndex};
    use crate::inode::IndexNode;

    pub const RAMFS_HEADER_SIZE: usize = size_of::<RamFs>();
    pub const RAMFS_DIR_ITEM_SIZE: usize = size_of::<DirItem>();
    pub const RAMFS_INODE_SIZE: usize = size_of::<IndexNode>();
    pub const RAMFS_INODE_INDEX_SIZE: usize = size_of::<InodeIndex>();
    pub const RAMFS_FRAME_INDEX_SIZE: usize = size_of::<FrameIndex>();
}

pub const RAMFS_MAX_DIR_ITEM_COUNT_PRE_FRAME: usize =
    RAMFS_FRAME_SIZE_BYTE / structs_size::RAMFS_DIR_ITEM_SIZE;

pub const RAMFS_ROOT_INODE_INDEX_USIZE: usize = 0;
pub const RAMFS_MAX_FILENAME_BYTE: usize = 31;
pub const RAMFS_PATH_DELIMITER: char = '/';

pub const RAMFS_FRAME_INDEXS_PRE_FRAME: usize =
    RAMFS_FRAME_SIZE_BYTE / structs_size::RAMFS_FRAME_INDEX_SIZE;

pub mod inode {
    use crate::constant::{RAMFS_FRAME_INDEXS_PRE_FRAME, RAMFS_FRAME_SIZE_BYTE};

    pub const RAMFS_ROOT_INODE_INDEX_USIZE: usize = 0;

    pub const RAMFS_FILENAME_BUFFER_SIZE: usize = 32;
    pub const RAMFS_MAX_FILENAME_BYTE: usize = RAMFS_FILENAME_BUFFER_SIZE - 1;

    pub const RAMFS_PAGE_1_COUNT: usize = 8;
    pub const RAMFS_PAGE_2_COUNT: usize = 1;
    pub const RAMFS_PAGE_3_COUNT: usize = 1;

    pub const RAMFS_FRAMES_PRE_PTR_1: usize = 1;
    pub const RAMFS_FRAMES_PRE_PTR_2: usize = RAMFS_FRAME_INDEXS_PRE_FRAME;
    pub const RAMFS_FRAMES_PER_PTR_3: usize =
        RAMFS_FRAME_INDEXS_PRE_FRAME * RAMFS_FRAME_INDEXS_PRE_FRAME;

    pub const RAMFS_FRAME_BOUND_1: usize = RAMFS_PAGE_1_COUNT * RAMFS_FRAMES_PRE_PTR_1;
    pub const RAMFS_FRAME_BOUND_2: usize =
        RAMFS_FRAME_BOUND_1 + RAMFS_PAGE_2_COUNT * RAMFS_FRAMES_PRE_PTR_2;
    pub const RAMFS_FRAME_BOUND_3: usize =
        RAMFS_FRAME_BOUND_2 + RAMFS_PAGE_3_COUNT * RAMFS_FRAMES_PER_PTR_3;
    
    pub const RAMFS_MAX_FILE_SIZE_FRAMES: usize = RAMFS_FRAME_BOUND_3;
    pub const RAMFS_MAX_FILE_SIZE_BYTE: usize = RAMFS_MAX_FILE_SIZE_FRAMES * RAMFS_FRAME_SIZE_BYTE;
}
