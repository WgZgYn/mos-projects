use crate::dentry::Dentry;
use alloc::sync::Arc;
use alloc::vec::Vec;
use mos_arch::sync::SpinIrqLock;
use vfs_common::{VfsError, VfsResult};

pub struct MountTable {
    set: SpinIrqLock<Vec<Arc<Dentry>>>,
}

pub static MOUNT_TABLE: MountTable = MountTable {
    set: SpinIrqLock::new(Vec::new()),
};

impl MountTable {
    pub fn mount(mount_point: Arc<Dentry>, root: Arc<Dentry>) -> VfsResult<()> {
        mount_point.mount(&root)?;
        MOUNT_TABLE.set.access(|t| t.push(root));
        Ok(())
    }
    pub fn unmount(root: Arc<Dentry>) -> VfsResult<()> {
        MOUNT_TABLE.set.access(|t| {
            let index = t
                .iter()
                .enumerate()
                .find(|&(_, dentry)| Arc::ptr_eq(dentry, &root))
                .map(|(i, _)| i)
                .ok_or(VfsError::NotFound);
            if let Ok(index) = index {
                t.swap_remove(index);
            }
        });
        Ok(())
    }
}
