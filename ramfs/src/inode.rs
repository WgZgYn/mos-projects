use crate::fs::{name_of, Error, FileType};
use crate::index::{FramePtr, InodeIndex};
use crate::utils::set_filename;
use kernel_utils::info;

#[repr(C)]
pub struct IndexNode {
    pub file_type: FileType,
    pub name: [u8; 32], // 32B
    pub parent: InodeIndex,
    pub size: u64,
    pub page_1: [FramePtr; 8], // store index but not address, currently dir only use the first direct page
    pub page_2: FramePtr,
    pub page_3: FramePtr,
    // RAMFS_FILE_MAX_SIZE_BYTE
}

impl IndexNode {
    pub fn init(
        &mut self,
        file_type: FileType,
        name: &str,
        parent: InodeIndex,
    ) -> Result<(), Error> {
        self.file_type = file_type;
        set_filename(&mut self.name, name)?;
        self.parent = parent;
        self.size = 0;
        self.page_1
            .iter_mut()
            .for_each(|item| *item = FramePtr::none());
        self.page_2 = FramePtr::none();
        self.page_3 = FramePtr::none();
        Ok(())
    }
    pub fn nth_frame(&self, i: usize) -> FramePtr {
        if i >= 8 {
            info!("Currently Not Supported multiple level frames");
            return FramePtr::none();
        }
        self.page_1[i]
    }
    pub fn nth_frame_mut(&mut self, i: usize) -> &mut FramePtr {
        if i >= 8 {
            panic!("Currently Not Supported multiple level frames");
        }
        &mut self.page_1[i]
    }

    pub fn name(&self) -> Option<&str> {
        name_of(&self.name)
    }
}
