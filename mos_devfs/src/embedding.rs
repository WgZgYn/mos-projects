use crate::DEV_DIR_INODE;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use vfs_common::{FileType, IoContext, VfsError, VfsResult};
use vfs_traits::*;

const RAW_BPP: &[u8] = include_bytes!("../../libs/vga/src/video_320x200_1bpp.bin");

pub struct EmbeddingInode;

lazy_static! {
    pub static ref EMBEDDING_INODE: Arc<EmbeddingInode> = Arc::new(EmbeddingInode);
}

impl FileOps for EmbeddingInode {
    fn read(&self, _ctx: &IoContext, offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        if offset >= RAW_BPP.len() {
            return Ok(0);
        }
        let n = buf.len().min(RAW_BPP.len() - offset);
        buf[..n].copy_from_slice(&RAW_BPP[offset..offset + n]);
        Ok(n)
    }

    fn write(&self, _ctx: &IoContext, _offset: usize, _buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }
}

impl Inode for EmbeddingInode {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn meta_data(&self) -> &[u8] {
        RAW_BPP
    }
    fn file_size(&self) -> u64 {
        RAW_BPP.len() as u64
    }
    fn file_type(&self) -> FileType {
        FileType::Regular
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
}
