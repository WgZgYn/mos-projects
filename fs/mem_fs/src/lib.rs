#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use vfs::{DirEntry, FileType, VfsError, VfsResult};
use vfs::traits::*;

lazy_static! {
    pub static ref FS: SuperBlock = SuperBlock {
        magic: "memfs",
        root: Arc::new(MemInode {
            inner: Mutex::new(InodeInner {
                name: String::from("/"),
                parent: None,
                content: FileContent::new(FileType::Directory),
            }),
        }),
    };
}

pub fn get_memfs_root() -> Arc<dyn Inode> {
    FS.root.clone()
}

enum FileContent {
    Dir(Vec<DirEntry>),
    File(Vec<u8>),
}

impl FileContent {
    const fn new(file_type: FileType) -> Self {
        match file_type {
            FileType::Regular => FileContent::File(Vec::new()),
            FileType::Directory => FileContent::Dir(Vec::new()),
            _ => panic!("not supported"),
        }
    }
    fn size(&self) -> usize {
        match &self {
            FileContent::Dir(d) => d.len() as _,
            FileContent::File(f) => f.len() as _,
        }
    }
    fn file_type(&self) -> FileType {
        match &self {
            FileContent::Dir(_) => FileType::Directory,
            FileContent::File(_) => FileType::Regular,
        }
    }
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>> {
        match self {
            FileContent::Dir(d) => d
                .iter()
                .find(|f| f.name() == name)
                .map(|f| f.node())
                .ok_or(VfsError::NotFound),
            FileContent::File(_) => Err(VfsError::NotDir),
        }
    }
    fn add_entry(&mut self, entry: DirEntry) -> VfsResult<()> {
        if let FileContent::Dir(d) = self {
            d.push(entry);
            Ok(())
        } else {
            Err(VfsError::NotDir)
        }
    }
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        if let FileContent::Dir(d) = self {
            Ok(d.get(offset).cloned())
        } else {
            Err(VfsError::NotDir)
        }
    }
}

pub struct SuperBlock {
    magic: &'static str,
    root: Arc<MemInode>,
}

struct InodeInner {
    name: String,
    parent: Option<Arc<MemInode>>,
    content: FileContent,
}

impl InodeInner {
    fn new(name: String, parent: Option<Arc<MemInode>>, content: FileContent) -> Self {
        Self {
            name,
            parent,
            content,
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
}

struct MemInode {
    inner: Mutex<InodeInner>,
}

impl Inode for MemInode {
    fn meta_data(&self) -> &[u8] {
        &[]
    }

    fn file_size(&self) -> u64 {
        self.inner.lock().content.size() as _
    }

    fn file_type(&self) -> FileType {
        self.inner.lock().content.file_type()
    }
}

impl InodeOps for MemInode {
    fn parent(&self) -> Option<Arc<dyn Inode>> {
        self.inner
            .lock()
            .parent
            .clone()
            .map::<Arc<dyn Inode>, _>(|e| e)
    }

    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>> {
        self.inner.lock().content.lookup(name)
    }

    fn create(self: Arc<Self>, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>> {
        let mut guard = self.inner.lock();
        if guard.content.file_type() != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        let node = Arc::new(MemInode {
            inner: Mutex::new(InodeInner::new(
                name.to_string(),
                Some(self.clone()),
                FileContent::new(file_type),
            )),
        });
        let entry = DirEntry::new(name.to_string(), file_type, node.clone());
        guard.content.add_entry(entry)?;
        Ok(node)
    }

    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        self.inner.lock().content.readdir(offset)
    }
}

impl FileOps for MemInode {
    fn read_at(&self, offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        if let FileContent::File(f) = &mut self.inner.lock().content {
            if f.len() > offset {
                let max_read = buf.len().min(f.len() - offset);
                buf[..max_read].copy_from_slice(&f[offset..offset + max_read]);
                Ok(max_read)
            } else {
                Ok(0)
            }
        } else {
            Err(VfsError::NotFile)
        }
    }

    fn write_at(&self, offset: usize, buf: &[u8]) -> VfsResult<usize> {
        if let FileContent::File(f) = &mut self.inner.lock().content {
            f.resize(offset + buf.len(), 0);
            f[offset..offset + buf.len()].copy_from_slice(buf);
            Ok(buf.len())
        } else {
            Err(VfsError::NotFile)
        }
    }
}
