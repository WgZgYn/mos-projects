use crate::constant::inode;
use crate::fs::{Error, FileType, name_of};
use crate::index::{FramePtr, InodeIndex};
use crate::utils::set_filename;

#[repr(C)]
pub struct IndexNode {
    pub file_type: FileType,
    pub name: [u8; inode::RAMFS_FILENAME_BUFFER_SIZE], // 32B, the last must be '\0', so MAX 31 legal ascii byte char
    pub parent: InodeIndex,
    pub index: InodeIndex,
    pub size: u64,
    pub page_1: [FramePtr; inode::RAMFS_PAGE_1_COUNT], // store index but not address, currently dir only use the first direct page
    pub page_2: [FramePtr; inode::RAMFS_PAGE_2_COUNT],
    pub page_3: [FramePtr; inode::RAMFS_PAGE_3_COUNT],
}

impl IndexNode {
    pub fn init(
        &mut self,
        file_type: FileType,
        name: &str,
        parent: InodeIndex,
        index: InodeIndex,
    ) -> Result<(), Error> {
        self.file_type = file_type;
        set_filename(&mut self.name, name)?;
        self.parent = parent;
        self.index = index;
        self.size = 0;
        self.page_1
            .iter_mut()
            .for_each(|item| *item = FramePtr::none());
        self.page_2
            .iter_mut()
            .for_each(|item| *item = FramePtr::none());
        self.page_3
            .iter_mut()
            .for_each(|item| *item = FramePtr::none());
        Ok(())
    }
    pub fn name(&self) -> Option<&str> {
        name_of(&self.name)
    }
}
