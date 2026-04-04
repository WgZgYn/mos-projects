use crate::constant::{
    RAMFS_FRAME_SIZE_BYTE, RAMFS_MAGIC, RAMFS_ROOT_INODE_INDEX_USIZE, structs_size,
};
use crate::fs::{FileSystemError, FileType, RamFs};
use crate::index::{InodeIndex, RAMFS_ROOT_INODE_INDEX};
use alloc::collections::BTreeMap;
use alloc::sync::Weak;
use core::ops::Deref;
use log::{info, warn};
use spin::Mutex;
use vfs_traits::Inode;

const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}
const fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}
const fn is_aligned(addr: u64, align: u64) -> bool {
    addr & (align - 1) == 0
}

pub struct RamFsManager<'a> {
    pub access: Mutex<&'a mut RamFs>,
    pub inodes: Mutex<BTreeMap<InodeIndex, Weak<dyn Inode>>>,
}

impl<'a> RamFsManager<'a> {
    fn check_load(meta: &'a mut [u8]) -> Result<&'a mut RamFs, FileSystemError> {
        // should pass aligned mem field
        if !is_aligned(meta.as_ptr() as u64, 4096) {
            return Err(FileSystemError::AlignError);
        }
        // check whether could trans to Ramfs Header
        if meta.len() < structs_size::RAMFS_HEADER_SIZE {
            return Err(FileSystemError::SizeError);
        }
        unsafe {
            let ramfs = &*(meta.as_ptr() as *const RamFs);
            // check magic
            if ramfs.magic() != RAMFS_MAGIC {
                return Err(FileSystemError::MagicError);
            }
            // check fs size
            if ramfs.fs_size() != meta.len() as u64 {
                return Err(FileSystemError::SizeError);
            }
            Ok(&mut *(meta.as_mut_ptr() as *mut RamFs))
        }
    }
    pub fn load(meta: &'a mut [u8]) -> Result<Self, FileSystemError> {
        let ramfs = Self::check_load(meta)?;
        Ok(Self {
            access: Mutex::new(ramfs),
            inodes: Mutex::new(BTreeMap::new()),
        })
    }
    pub fn build(meta: &'a mut [u8]) -> Result<Self, FileSystemError> {
        // aligned the meta data
        let fs_start = align_up(meta.as_ptr() as u64, 4096);

        // after aligned, the meta is unusable
        if fs_start >= meta.as_ptr() as u64 + meta.len() as u64 {
            return Err(FileSystemError::SizeError);
        }

        // warning
        let offset = fs_start - meta.as_ptr() as u64;
        if offset != 0 {
            warn!("meta is not page aligned, {} Byte will be wasted", offset);
        }

        // clear to zero
        meta.fill(0);
        let meta = &mut meta[offset as usize..];

        // check minimal size
        if meta.len() < size_of::<RamFs>() {
            return Err(FileSystemError::SizeError);
        }

        // cast to ramfs header
        let buffer_size = meta.len();
        let header = unsafe { &mut *(meta.as_mut_ptr() as *mut RamFs) };

        header.magic = RAMFS_MAGIC;
        header.fs_size = buffer_size as u64;

        header.frame_bitmap.set_all(); // means all unusable
        let frame_start = align_up(
            header.fs_start() + structs_size::RAMFS_HEADER_SIZE as u64,
            RAMFS_FRAME_SIZE_BYTE as _,
        );
        header.first_frame_offset = frame_start - header.fs_start();

        let frame_count = header.total_frames() as _;
        header.frame_bitmap.clear_range(0..frame_count);
        info!("total {} frames free", frame_count);

        // create the root dir -- 'ramfs'
        header.inode_bitmap.set(RAMFS_ROOT_INODE_INDEX_USIZE);
        let root_inode = &mut header.inodes[RAMFS_ROOT_INODE_INDEX_USIZE];
        root_inode
            .init(
                FileType::Directory,
                "ramfs",
                RAMFS_ROOT_INODE_INDEX,
                RAMFS_ROOT_INODE_INDEX,
            )
            .map_err(|_| FileSystemError::BuildError)?;

        Ok(Self {
            access: Mutex::new(header),
            inodes: Mutex::new(BTreeMap::new()),
        })
    }
}

impl<'a> Deref for RamFsManager<'a> {
    type Target = Mutex<&'a mut RamFs>;
    fn deref(&self) -> &Self::Target {
        &self.access
    }
}
