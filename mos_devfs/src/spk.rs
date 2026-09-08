use crate::DEV_DIR_INODE;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_driver::pcspk::speaker_on;
use vfs_common::{FileType, IoContext, VfsError, VfsResult};
use vfs_traits::{FileOps, Inode};

pub struct PcSpeaker;

lazy_static! {
    pub static ref SPK_INODE: Arc<PcSpeaker> = Arc::new(PcSpeaker);
}

impl Inode for PcSpeaker {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn file_type(&self) -> FileType {
        FileType::CharDevice
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
}

impl FileOps for PcSpeaker {
    fn read(&self, _ctx: &IoContext, _offset: usize, _buf: &mut [u8]) -> VfsResult<usize> {
        Err(VfsError::NotSupported)
    }

    fn write(&self, _ctx: &IoContext, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        if buf.len() != 4 {
            return Err(VfsError::NotSupported);
        }
        let freq = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
        speaker_on(freq);
        Ok(4)
    }
}
