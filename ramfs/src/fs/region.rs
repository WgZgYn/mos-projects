use crate::constant::{RAMFS_FRAME_SIZE_BYTE, RAMFS_HEADER_SIZE_BYTE};
use crate::fs::{FileSystemError, RamFs, RamFsMappedRegion};
use core::ptr::NonNull;

impl RamFsMappedRegion {
    pub fn new(start: u64, size: usize) -> Result<Self, FileSystemError> {
        if !start.is_multiple_of(RAMFS_FRAME_SIZE_BYTE as u64) {
            return Err(FileSystemError::AlignError);
        }

        if size < RAMFS_HEADER_SIZE_BYTE {
            return Err(FileSystemError::SizeError);
        }

        Ok(Self {
            start: NonNull::new(start as *mut u8).ok_or(FileSystemError::LoadError)?,
            size,
        })
    }
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { core::slice::from_raw_parts(self.start.as_ptr(), self.size) }
    }
    pub fn as_bytes_mut(&mut self) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.start.as_ptr(), self.size) }
    }
    pub fn as_header(&self) -> &'static RamFs {
        unsafe { &*(self.start.as_ptr() as *mut RamFs) }
    }
    pub fn as_header_mut(&mut self) -> &'static mut RamFs {
        unsafe { &mut *(self.start.as_ptr() as *mut RamFs) }
    }
    pub fn region_start(&self) -> u64 {
        self.start.as_ptr() as u64
    }
    pub fn region_size(&self) -> u64 {
        self.size as u64
    }
}
