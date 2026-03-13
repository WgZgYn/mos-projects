#![no_std]

extern crate alloc;
mod file;
mod path;
mod traits;

use crate::path::{AsPath, Component};
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use kernel_utils::{bitflags, info, string_enum, RwLock};

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
    #[derive(Copy, Clone, Eq, PartialEq)]
    pub struct OpenFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const READ_WRITE = OpenFlags::READ.bits() | OpenFlags::WRITE.bits();
        const CREATE = 1 << 2;
        const APPEND = 1 << 3;
        const TRUNC = 1 << 4;
        const DIRECTORY = 1 << 5;
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

pub struct DentryInner {
    name: String,
    inode: Option<Arc<dyn Inode>>,
    children: BTreeMap<String, Arc<Dentry>>,

    parent: Weak<Dentry>,

    // reverse-dependency
    mounted: Weak<Dentry>,
    mount_point: Option<Arc<Dentry>>,
}

pub struct Dentry {
    inner: RwLock<DentryInner>,
}

impl Dentry {
    /// should only have one root for one fs
    pub unsafe fn create_root(inode: Arc<dyn Inode>) -> Arc<Self> {
        Arc::new(Self::new(DentryInner::new(
            "/".to_owned(),
            Weak::new(),
            Some(inode),
        )))
    }
    fn new(inner: DentryInner) -> Self {
        Self {
            inner: RwLock::new(inner),
        }
    }
    pub fn name(&self) -> String {
        self.inner.read().name().to_owned()
    }
    pub fn inode(&self) -> Option<Arc<dyn Inode>> {
        self.inner.read().inode.clone()
    }
    pub fn file_type(&self) -> VfsResult<FileType> {
        self.inner.read().file_type()
    }
    pub fn file_size(&self) -> VfsResult<u64> {
        self.inner.read().file_size()
    }
    pub fn parent(&self) -> Option<Arc<Dentry>> {
        self.inner.read().parent()
    }
    pub fn mount(self: Arc<Self>, root: &Arc<Self>) -> VfsResult<()> {
        mount(self, root)
    }
    pub fn lookup(self: &Arc<Self>, path: &str) -> VfsResult<Arc<Self>> {
        fn lookup_from_inode(
            parent: Weak<Dentry>,
            inode: &Arc<dyn Inode>,
            path: &str,
        ) -> VfsResult<Arc<Dentry>> {
            inode.lookup(path).and_then(|child| {
                let inner = DentryInner::new(path.to_owned(), parent, Some(child));
                Ok(Arc::new(Dentry::new(inner)))
            })
        }

        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };

        let ret = inner.children.get(path).cloned();
        drop(inner);

        match ret {
            None => {
                if let Ok(child) = lookup_from_inode(Arc::downgrade(self), &inode, path) {
                    self.inner
                        .write()
                        .children
                        .insert(path.to_owned(), child.clone());
                    Ok(child)
                } else {
                    Err(VfsError::NotFound)
                }
            }
            Some(child) => Ok(child.follow_mount()),
        }
    }
    pub fn create(self: &Arc<Self>, path: &str, file_type: FileType) -> VfsResult<Arc<Self>> {
        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };
        if inner.children.contains_key(path) || inode.lookup(path).is_ok() {
            return Err(VfsError::AlreadyExists);
        }
        drop(inner);
        let new_inode = inode.create(path, file_type)?;
        let parent = Arc::downgrade(self);
        let dentry = Arc::new(Dentry::new(DentryInner::new(
            path.to_owned(),
            parent,
            Some(new_inode),
        )));
        let mut inner = self.inner.write();
        inner.children.insert(path.to_owned(), dentry.clone());
        Ok(dentry)
    }
    pub fn open(self: &Arc<Self>, flags: OpenFlags) -> VfsResult<File> {
        let inode = self.inode().ok_or(VfsError::NotFound)?;
        let offset = if flags.contains(OpenFlags::APPEND) {
            inode.file_size()
        } else {
            0
        };
        File::new(self.clone(), offset, flags)
    }
    pub fn follow_mount(self: Arc<Self>) -> Arc<Self> {
        self.inner
            .read()
            .mounted
            .upgrade()
            .map(|mount_point| mount_point.follow_mount())
            .unwrap_or(self.clone())
    }
    pub fn follow_dotdot(self: Arc<Self>) -> Option<Arc<Self>> {
        self.inner
            .read()
            .mount_point
            .clone()
            .and_then(|mount_point| mount_point.follow_dotdot())
            .or_else(|| self.parent())
    }
}

impl DentryInner {
    pub fn new(name: String, parent: Weak<Dentry>, inode: Option<Arc<dyn Inode>>) -> Self {
        Self {
            name,
            inode,
            children: BTreeMap::new(),
            parent,
            mounted: Weak::new(),
            mount_point: None,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn file_type(&self) -> VfsResult<FileType> {
        self.inode
            .as_ref()
            .map(|inode| inode.file_type())
            .ok_or(VfsError::NotFound)
    }
    pub fn parent(&self) -> Option<Arc<Dentry>> {
        self.parent.upgrade()
    }
    pub fn file_size(&self) -> VfsResult<u64> {
        self.inode
            .as_ref()
            .map(|inode| inode.file_size())
            .ok_or(VfsError::NotFound)
    }
}

fn mount(mount_point: Arc<Dentry>, root: &Arc<Dentry>) -> VfsResult<()> {
    if let FileType::Directory = mount_point.file_type()? {
        root.inner.write().mount_point = Some(mount_point.clone());
        mount_point.inner.write().mounted = Arc::downgrade(root);
        Ok(())
    } else {
        Err(VfsError::NotDir)
    }
}
