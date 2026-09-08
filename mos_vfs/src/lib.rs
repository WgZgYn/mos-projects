#![no_std]
extern crate alloc;

use crate::dentry::Dentry;
use crate::file::File;
use crate::mount::MountTable;
use crate::vnode::{InodeExt, VfsInode};
use alloc::sync::Arc;
use conquer_once::spin::OnceCell;
use log::info;
use vfs_common::path::{AsPath, Component};
use vfs_common::*;
use vfs_traits::*;

pub mod dentry;
pub mod file;
mod mount;
pub mod vnode;

pub static VFS_ROOT: OnceCell<Arc<VfsInode>> = OnceCell::uninit();
pub static VFS_ROOT_DENTRY: OnceCell<Arc<Dentry>> = OnceCell::uninit();

pub fn vfs_root_inode() -> Arc<VfsInode> {
    VFS_ROOT.get().expect("VFS not set").clone()
}
pub fn vfs_root_dentry() -> Arc<Dentry> {
    VFS_ROOT_DENTRY.get().expect("VFS not set").clone()
}
pub fn register_root_inode(inode: Arc<dyn Inode>) {
    let root = Arc::new(VfsInode::new(inode.clone()));
    VFS_ROOT.init_once(|| root.clone());
    VFS_ROOT_DENTRY.init_once(|| Dentry::create_root(root));
}

pub fn dir(path: &str) -> VfsResult<DirIter> {
    vfs_open(
        vfs_root_dentry(),
        vfs_root_dentry(),
        path,
        OpenFlags::READ | OpenFlags::DIRECTORY,
    )
    .and_then(|f| match f {
        OpenResult::File(f) => Ok(DirIter::new(f)),
        _ => Err(VfsError::NotDir),
    })
}
pub fn mount(mount_point: Arc<Dentry>, root: Arc<Dentry>) -> VfsResult<()> {
    MountTable::mount(mount_point, root)
}
pub fn vfs_create(
    root: Arc<Dentry>,
    curr: Arc<Dentry>,
    path: impl AsPath,
    file_type: FileType,
) -> VfsResult<Arc<Dentry>> {
    let result = vfs_lookup(root, curr, &path);
    match result {
        VfsLookUpResult::Ok(_) => Err(VfsError::AlreadyExists),
        VfsLookUpResult::Last(dir, name) => dir.create(name, file_type),
        _ => Err(VfsError::NotFound),
    }
}

pub enum VfsLookUpResult<'a> {
    Ok(Arc<Dentry>),
    Last(Arc<Dentry>, &'a str),
    Err(VfsError),
}

/// maybe need mode to check permission
pub fn vfs_lookup(
    root: Arc<Dentry>, // must be dir
    curr: Arc<Dentry>, // must be dir
    path: &'_ impl AsPath,
) -> VfsLookUpResult<'_> {
    let mut curr = curr;
    let mut path = path.as_path().peekable();

    while let Some(comp) = path.next() {
        match curr.file_type() {
            Err(e) => return VfsLookUpResult::Err(e),
            Ok(f) if f != FileType::Directory => return VfsLookUpResult::Err(VfsError::NotDir),
            _ => {}
        }

        curr = match comp {
            Component::Root => root.clone(),
            Component::Current => continue,
            Component::Parent => curr.follow_dotdot().unwrap_or(root.clone()),
            Component::Normal(name) => match curr.lookup(name) {
                Ok(node) => node,
                Err(VfsError::NotFound) => {
                    return if path.peek().is_none() {
                        VfsLookUpResult::Last(curr, name)
                    } else {
                        VfsLookUpResult::Err(VfsError::NotFound)
                    };
                }
                Err(e) => return VfsLookUpResult::Err(e),
            },
        }
    }
    VfsLookUpResult::Ok(curr)
}

pub struct KernelOpenFile {
    dentry: Arc<Dentry>,
    flags: OpenFlags,
}

impl KernelOpenFile {
    pub fn open_with(self, ext: InodeExt) -> VfsResult<File> {
        let inode = self.dentry.inode().ok_or(VfsError::NotFound)?;
        assert!(!inode.has_ext());
        inode.set_ext(ext);
        File::from_inode(inode.inode(), self.dentry, 0, self.flags)
    }
    pub fn file_type(&self) -> VfsResult<FileType> {
        self.dentry.file_type()
    }
}

pub enum OpenResult {
    File(File),
    FirstOpenLogicFile(KernelOpenFile),
}

pub fn vfs_open(
    root: Arc<Dentry>,
    cur: Arc<Dentry>,
    path: &str,
    flags: OpenFlags,
) -> VfsResult<OpenResult> {
    let result = vfs_lookup(root, cur, &path);
    let dentry = match result {
        VfsLookUpResult::Ok(inode) => inode,
        VfsLookUpResult::Err(e) => return Err(e),
        VfsLookUpResult::Last(dentry, name) => {
            if flags.contains(OpenFlags::CREATE) {
                let ft = if flags.contains(OpenFlags::DIRECTORY) {
                    FileType::Directory
                } else {
                    FileType::Regular
                };
                dentry.create(name, ft)?
            } else {
                return Err(VfsError::NotFound);
            }
        }
    };
    vfs_open_dentry(dentry, flags)
}

pub fn vfs_open_dentry(dentry: Arc<Dentry>, flags: OpenFlags) -> VfsResult<OpenResult> {
    let inode = dentry.inode().ok_or(VfsError::NotFound)?;
    let ft = inode.file_type();
    match ft {
        FileType::Directory
        | FileType::Regular
        | FileType::BlockDevice
        | FileType::CharDevice
        | FileType::SymLink => {
            let offset = if flags.contains(OpenFlags::APPEND) {
                inode.file_size()
            } else {
                0
            };
            Ok(OpenResult::File(File::from_dentry(dentry, offset, flags)?))
        }
        FileType::Fifo => {
            if inode.has_ext() {
                Ok(OpenResult::File(File::from_inode(
                    inode.inode(),
                    dentry,
                    0,
                    flags,
                )?))
            } else {
                Ok(OpenResult::FirstOpenLogicFile(KernelOpenFile {
                    dentry,
                    flags,
                }))
            }
        }
    }
}

pub struct DirIter {
    file: File,
    index: usize,
}

impl DirIter {
    pub fn new(file: File) -> Self {
        Self { file, index: 0 }
    }
}

impl Iterator for DirIter {
    type Item = DirEntry;
    fn next(&mut self) -> Option<Self::Item> {
        self.index += 1;
        self.file.read_dir(self.index - 1).ok()?
    }
}

fn is_legal_filename(name: &str) -> bool {
    info!("{:?}", name);
    name.len() < 32
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}
