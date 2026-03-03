#![no_std]

extern crate alloc;
mod file;
mod path;
mod traits;

use crate::path::{AsPath, Component};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use kernel_utils::{bitflags, string_enum, RwLock};

pub use crate::file::{File, FileRef, RegularFile};

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
        FileNameIllegal,
        NotOpen,
        BadFileDesc,
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
        const DIRECTORY = 32;
    }
}

#[derive(Clone)]
pub struct DirEntry {
    name: String,
    ft: FileType,
    node: Arc<dyn Inode>,
}

#[derive(Clone)]
pub struct UserDirEntry {
    pub name: [u8; 32],
    pub file_type: FileType,
    pub size: u64,
}

impl UserDirEntry {
    pub fn from(entry: &DirEntry) -> Self {
        let mut name = [0_u8; 32];
        let bs = entry.name().as_bytes();
        for i in 0..31.min(bs.len()) {
            name[i] = bs[i];
        }
        Self {
            name,
            file_type: entry.file_type(),
            size: entry.node.file_size(),
        }
    }
    pub fn name(&self) -> Option<&str> {
        let mut i = 0;
        while i < self.name.len() {
            if self.name[i] == 0 {
                return core::str::from_utf8(&self.name[..i]).ok();
            }
            i += 1;
        }
        None
    }
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
    fn create(self: Arc<Self>, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>>;
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>>;
}

pub trait FileHandler: Send + Sync {
    fn read(&self, buf: &mut [u8]) -> VfsResult<usize>;
    fn write(&self, buf: &[u8]) -> VfsResult<usize>;
    fn read_dir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        Err(VfsError::NotDir)
    }
}

fn is_legal_filename(name: &str) -> bool {
    name.len() < 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

pub fn vfs_open(
    root: Arc<dyn Inode>,
    cur: Arc<dyn Inode>,
    path: &str,
    flags: OpenFlags,
) -> VfsResult<File> {
    let mut cur = cur;
    for comp in path.as_path() {
        if cur.file_type() != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        cur = match comp {
            Component::Root => root.clone(),
            Component::Current => continue,
            Component::Parent => cur.parent().unwrap_or(root.clone()),
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
    let offset = if flags.contains(OpenFlags::APPEND) {
        cur.file_size()
    } else {
        0
    };
    let file = File::new(cur, offset, flags);
    Ok(file)
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
