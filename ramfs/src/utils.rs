use crate::constant::RAMFS_MAX_FILENAME_BYTE;
use crate::fs::Error;
use crate::fs_manager::RamFsManager;
use crate::index::InodeIndex;
use crate::inode::IndexNode;
use alloc::vec::Vec;
use kernel_utils::error;

pub struct FsTransaction<'a, 'b> {
    header: &'a RamFsManager<'b>,
    inode: Vec<(InodeIndex, Option<IndexNode>)>,
    success: bool,
}

impl<'a, 'b> FsTransaction<'a, 'b> {
    pub fn new(header: &'a RamFsManager<'b>) -> FsTransaction<'a, 'b> {
        Self {
            header,
            inode: Vec::new(),
            success: false,
        }
    }
    pub fn backup(&mut self, inode: InodeIndex, node: Option<IndexNode>) {
        self.inode.push((inode, node));
    }
    pub fn success(&mut self) {
        self.success = true;
    }
    pub fn is_success(&self) -> bool {
        self.success
    }
}

impl<'a, 'b> Drop for FsTransaction<'a, 'b> {
    fn drop(&mut self) {
        if self.success {
            return;
        }
        for (idx, node) in &mut self.inode {
            match node.take() {
                // make it unused
                None => {
                    let t = self.header.lock().free_inode(*idx);
                    if let Err(e) = t {
                        error!("{}", e.as_str())
                    }
                }
                // rollback
                Some(node) => self.header.lock()[*idx] = node,
            }
        }
    }
}


pub(crate) fn check_filename(name: &str) -> Result<(), Error> {
    if name.is_empty() {
        return Err(Error::FileNameIllegal);
    }
    if name.len() > RAMFS_MAX_FILENAME_BYTE {
        return Err(Error::FileNameTooLong);
    }
    if name.contains(|c: char| !c.is_ascii() || c.is_ascii_control() || c == '/' || c == '\\') {
        return Err(Error::FileNameIllegal);
    }
    Ok(())
}

pub(crate) fn set_filename(dst: &mut [u8; 32], name: &str) -> Result<(), Error> {
    check_filename(name)?;
    let (data, zero) = dst.split_at_mut(name.len());
    data.copy_from_slice(name.as_bytes());
    zero.fill(0);
    Ok(())
}
