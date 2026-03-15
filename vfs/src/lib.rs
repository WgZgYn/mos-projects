#![no_std]

extern crate alloc;
mod dentry;
mod file;
pub mod traits;

pub use crate::dentry::Dentry;
pub use crate::file::File;

use alloc::string::String;
use alloc::sync::Arc;
use log::info;

use crate::traits::{Inode, InodeOps};
use vfs_user::path::{AsPath, Component};
pub use vfs_user::*;

pub type VfsResult<T> = Result<T, VfsError>;

#[derive(Clone)]
pub struct DirEntry {
    name: String,
    ft: FileType,
    node: Arc<dyn Inode>,
}

impl DirEntry {
    pub fn new(name: String, ft: FileType, node: Arc<dyn Inode>) -> Self {
        DirEntry { name, ft, node }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn file_type(&self) -> FileType {
        self.ft
    }
    pub fn node(&self) -> Arc<dyn Inode> {
        self.node.clone()
    }
}

fn is_legal_filename(name: &str) -> bool {
    info!("{:?}", name);
    name.len() < 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

pub fn vfs_open(
    root: Arc<Dentry>,
    cur: Arc<Dentry>,
    path: &str,
    flags: OpenFlags,
) -> VfsResult<File> {
    let mut cur = cur;
    for comp in path.as_path() {
        if cur.file_type()? != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        cur = match comp {
            Component::Root => root.clone(),
            Component::Current => continue,
            Component::Parent => cur.follow_dotdot().unwrap_or(root.clone()),
            Component::Normal(path) => match cur.lookup(path) {
                Ok(node) => node,
                Err(VfsError::NotFound) => {
                    if !flags.contains(OpenFlags::CREATE) {
                        return Err(VfsError::NotFound);
                    }
                    if !is_legal_filename(path) {
                        return Err(VfsError::FileNameIllegal);
                    }
                    cur.create(
                        path,
                        if flags.contains(OpenFlags::DIRECTORY) {
                            FileType::Directory
                        } else {
                            FileType::Regular
                        },
                    )?
                }
                Err(e) => return Err(e),
            },
        }
    }
    cur.open(flags)
}

pub struct DirIter {
    node: Arc<dyn InodeOps>,
    index: usize,
}

impl DirIter {
    pub fn new(node: Arc<dyn InodeOps>) -> Self {
        Self { node, index: 0 }
    }
}

impl Iterator for DirIter {
    type Item = DirEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1;
        self.node.readdir(self.index - 1).ok()?
    }
}
