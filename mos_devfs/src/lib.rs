#![no_std]
mod embedding;
mod null;
mod rand;
mod spk;
pub mod tty;
mod zero;

extern crate alloc;

use crate::embedding::EMBEDDING_INODE;
use crate::null::NULL_INODE;
use crate::rand::RAND_INODE;
use crate::spk::SPK_INODE;
use crate::tty::TTY_INODE;
use crate::zero::ZERO_INODE;
use alloc::borrow::ToOwned;
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_vfs::dentry::Dentry;
use mos_vfs::vnode::VfsInode;
pub use null::NullInode;
use vfs_common::{FileType, VfsError, VfsResult};
use vfs_traits::{DirEntry, DirOps, Inode};
pub use zero::ZeroInode;

pub struct DevDirInode;
lazy_static! {
    pub static ref DEV_DIR_INODE: Arc<DevDirInode> = Arc::new(DevDirInode);
    pub static ref DEV_DIR_DENTRY: Arc<Dentry> =
        Dentry::create_root(Arc::new(VfsInode::new(DEV_DIR_INODE.clone())));
}

impl DirOps for DevDirInode {
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>> {
        match name {
            "zero" => Ok(ZERO_INODE.clone()),
            "null" => Ok(NULL_INODE.clone()),
            "tty" => Ok(TTY_INODE.clone()),
            "rand" => Ok(RAND_INODE.clone()),
            "embedding" => Ok(EMBEDDING_INODE.clone()),
            "spk" => Ok(SPK_INODE.clone()),
            _ => Err(VfsError::NotFound),
        }
    }

    fn create(&self, _name: &str, _file_type: FileType) -> VfsResult<Arc<dyn Inode>> {
        Err(VfsError::NotSupported)
    }

    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        if let Some(&s) = ["null", "tty", "zero", "rand", "embedding", "spk"].get(offset) {
            let inode = self.lookup(s)?;
            let entry = DirEntry::new(s.to_owned(), inode.file_type(), inode);
            Ok(Some(entry))
        } else {
            Ok(None)
        }
    }
}

impl Inode for DevDirInode {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn file_type(&self) -> FileType {
        FileType::Directory
    }
    fn as_dir_ops(&self) -> VfsResult<&dyn DirOps> {
        Ok(self)
    }
}
