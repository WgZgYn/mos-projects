#![no_std]

mod path;
mod traits;

extern crate alloc;

use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};
use kernel_utils::{RwLock, bitflags};

// below is dynamic dispatch compatible
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

pub trait Inode: Send + Sync + FileOps + DirInode {
    fn meta_data(&self) -> &[u8];
    fn parent(&self) -> Option<Arc<dyn Inode>>;
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>>;
    fn create(&self, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn file_size(&self) -> u64;
    fn file_type(&self) -> FileType;
}

pub trait FileOps: Send + Sync {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize>;
    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize>;
}

pub trait DirInode {
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;
}

pub struct File {
    inode: Arc<dyn FileOps>,
    offset: AtomicUsize,
}

impl File {
    pub fn new(inode: Arc<dyn FileOps>) -> Self {
        Self {
            inode,
            offset: AtomicUsize::new(0),
        }
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        let off = self.offset.load(Ordering::Relaxed);
        let n = self.inode.read_at(off, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        let off = self.offset.load(Ordering::Relaxed);
        let n = self.inode.write_at(off, buf)?;
        self.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn seek(&self, offset: usize) {
        self.offset.store(offset, Ordering::SeqCst);
    }
    pub fn dup(self) -> DupFile {
        DupFile(Arc::new(self))
    }
}

#[derive(Clone)]
pub struct DupFile(Arc<File>);
impl Deref for DupFile {
    type Target = File;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct ReadOnlyFile(File);
impl ReadOnlyFile {
    fn new(file: File) -> Self {
        Self(file)
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        let off = self.0.offset.load(Ordering::Relaxed);
        let n = self.0.inode.read_at(off, buf)?;
        self.0.offset.fetch_add(n, Ordering::Relaxed);
        Ok(n)
    }
    pub fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        Err(VfsError::PermissionDenied)
    }
    pub fn seek(&self, offset: usize) {
        self.0.offset.store(offset, Ordering::SeqCst);
    }
}

pub fn lookup_path(path: &str, root: Arc<dyn Inode>) -> VfsResult<Arc<dyn Inode>> {
    let mut cur = root;
    for comp in path.split('/').filter(|s| !s.is_empty()) {
        if cur.file_type() != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        match comp {
            "." => continue,
            ".." => cur = cur.parent().unwrap_or(cur),
            _ => cur = cur.lookup(comp)?,
        }
    }
    Ok(cur)
}

pub fn vfs_open(root: Arc<dyn Inode>, path: &str, open_flags: OpenFlags) -> VfsResult<File> {
    let node = lookup_path(path, root)?;
    if node.file_type() != FileType::Directory && node.file_type() != FileType::Regular {
        return Err(VfsError::NotFile);
    }
    let mut offset = 0;
    if open_flags.contains(OpenFlags::APPEND) {
        offset = node.file_size() as usize;
    }
    Ok(File {
        inode: node,
        offset: AtomicUsize::new(offset),
    })
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
