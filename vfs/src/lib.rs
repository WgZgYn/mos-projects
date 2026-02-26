#![no_std]

mod file;
mod path;
mod traits;

extern crate alloc;

use crate::path::{AsPath, Component};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use kernel_utils::{RwLock, bitflags, string_enum};

pub use crate::file::File;

// below is dynamic dispatch compatible
string_enum!(
    #[derive(Debug)]
    pub enum VfsError {
        NotFound,
        NotDir,
        NotFile,
        NoSpace,
        PermissionDenied,
        IoError,
        AlreadyExists,
    }
);

pub type VfsResult<T> = Result<T, VfsError>;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum FileType {
    Regular,
    Directory,
    CharDevice(u32),
    BlockDevice(u32),
    Socket,
    Fifo,
    Link,
}

bitflags! {
    pub struct OpenFlags: u32 {
        const READ = 1;
        const WRITE = 2;
        const READ_WRITE = OpenFlags::READ.bits() | OpenFlags::WRITE.bits();
        const CREATE = 4;
        const APPEND = 8;
        const TRUNC = 16;
    }
}

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
}

bitflags! {
    pub struct MountFlags: u32 {

    }
}

pub trait VFS {
    fn name(&self) -> Option<&str>;
    fn mount(&self, dev: Option<&str>, flags: MountFlags) -> VfsResult<Arc<dyn SuperBlock>>;
}

pub trait SuperBlock {
    fn root(&self) -> Arc<dyn Inode>;
}

pub trait Inode: FileOps + InodeOps {
    fn meta_data(&self) -> &[u8];
    fn file_size(&self) -> u64;
    fn file_type(&self) -> FileType;
}

pub trait FileOps: Send + Sync {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize>;
}

pub trait InodeOps: Send + Sync {
    fn parent(&self) -> Option<Arc<dyn Inode>>;
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>>;
    fn create(&self, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;
}

pub fn lookup_path(path: &str, root: Arc<dyn Inode>) -> VfsResult<Arc<dyn Inode>> {
    let mut cur = root.clone();
    for comp in path.as_path() {
        if cur.file_type() != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        cur = match comp {
            Component::Root => root.clone(),
            Component::Current => continue,
            Component::Parent => cur.parent().unwrap_or(cur),
            Component::Normal(path) => cur.lookup(path)?,
        }
    }
    Ok(cur)
}

pub fn vfs_open(root: Arc<dyn Inode>, path: &str) -> VfsResult<Arc<dyn Inode>> {
    let node = lookup_path(path, root)?;
    if node.file_type() != FileType::Directory && node.file_type() != FileType::Regular {
        return Err(VfsError::NotFile);
    }
    Ok(node)
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

pub struct Dentry {
    name: String,
    inode: Arc<dyn Inode>,
    parent: Option<Arc<RwLock<Dentry>>>,
    children: RwLock<BTreeMap<String, Arc<RwLock<Dentry>>>>,
    mounted: Option<Arc<Mount>>,
}

impl Dentry {
    pub fn new(name: String, inode: Arc<dyn Inode>, parent: Option<Arc<RwLock<Dentry>>>) -> Self {
        Self {
            name,
            inode,
            parent,
            children: Default::default(),
            mounted: None,
        }
    }

    pub fn lookup(&self, name: &str) -> VfsResult<Arc<RwLock<Dentry>>> {
        self.children
            .read()
            .get(name)
            .cloned()
            .ok_or(VfsError::NotFound)
    }

    pub fn add_child(&self, name: &str, dentry: Arc<RwLock<Dentry>>) -> VfsResult<()> {
        let mut children = self.children.write();
        if children.contains_key(name) {
            return Err(VfsError::AlreadyExists);
        }
        children.insert(name.to_owned(), dentry);
        Ok(())
    }
}

pub struct Mount {
    root: Arc<dyn Inode>,
    mount: Arc<dyn Inode>,
}
