use alloc::sync::Arc;
use vfs_common::FileType;
use vfs_traits::Inode;

#[derive(Clone)]
pub enum InodeExt {
    Fifo(Arc<dyn Inode>),
    Socket(Arc<dyn Inode>),
}

/// vfs wrapped inode
pub struct VfsInode {
    inner: Arc<dyn Inode>,
    ext: spin::Mutex<Option<InodeExt>>,
}

impl VfsInode {
    pub const fn new(inner: Arc<dyn Inode>) -> Self {
        Self {
            inner,
            ext: spin::Mutex::new(None),
        }
    }

    pub fn set_ext(&self, ext: InodeExt) {
        *self.ext.lock() = Some(ext);
    }

    pub fn has_ext(&self) -> bool {
        self.ext.lock().is_some()
    }

    pub fn get_ext(&self) -> Option<InodeExt> {
        self.ext.lock().clone()
    }

    pub fn file_type(&self) -> FileType {
        self.inner.file_type()
    }

    pub fn file_size(&self) -> u64 {
        self.inner.file_size()
    }

    pub fn inode(&self) -> Arc<dyn Inode> {
        match self.get_ext() {
            None => self.inner.clone(),
            Some(InodeExt::Fifo(fifo)) => fifo,
            Some(InodeExt::Socket(sock)) => sock,
            _ => unimplemented!(),
        }
    }
}
