use crate::vnode::VfsInode;
use alloc::borrow::ToOwned;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::{Arc, Weak};
use spin::RwLock;
use vfs_common::{FileType, VfsError, VfsResult};

pub struct DentryInner {
    name: String,
    inode: Option<Arc<VfsInode>>,
    children: BTreeMap<String, Arc<Dentry>>,
    parent: Weak<Dentry>,
    mounted: Weak<Dentry>,
    mount_point: Option<Arc<Dentry>>,
}

pub struct Dentry {
    inner: RwLock<DentryInner>,
}

impl Dentry {
    pub fn new_anonymous(inode: Arc<VfsInode>) -> Arc<Self> {
        Arc::new(Self::new(DentryInner::new(
            String::new(),
            Weak::new(),
            Some(inode),
        )))
    }
    /// should only have one root for one fs, inode must be a root dir
    pub fn create_root(inode: Arc<VfsInode>) -> Arc<Self> {
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
    pub fn inode(&self) -> Option<Arc<VfsInode>> {
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

    /// Mount roots have an internal empty name; getcwd must use the visible
    /// mountpoint before walking the parent chain.
    pub fn path_location(self: &Arc<Self>) -> (String, Option<Arc<Dentry>>) {
        let inner = self.inner.read();
        if let Some(mountpoint) = inner.mount_point.clone() {
            drop(inner);
            mountpoint.path_location()
        } else {
            (inner.name.clone(), inner.parent())
        }
    }
    pub fn mount(self: Arc<Self>, root: &Arc<Self>) -> VfsResult<()> {
        mount(self, root)
    }
    pub fn lookup(self: &Arc<Self>, path: &str) -> VfsResult<Arc<Self>> {
        fn lookup_from_inode(
            parent: Weak<Dentry>,
            inode: &Arc<VfsInode>,
            path: &str,
        ) -> VfsResult<Arc<Dentry>> {
            inode.inode().as_dir_ops()?.lookup(path).and_then(|child| {
                let inner = DentryInner::new(
                    path.to_owned(),
                    parent,
                    Some(Arc::new(VfsInode::new(child))),
                );
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
    pub fn create(self: &Arc<Self>, name: &str, file_type: FileType) -> VfsResult<Arc<Self>> {
        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };
        let dir_inode = inode.inode();
        let dir_inode = dir_inode.as_dir_ops()?;
        if inner.children.contains_key(name) || dir_inode.lookup(name).is_ok() {
            return Err(VfsError::AlreadyExists);
        }
        drop(inner);
        let new_inode = dir_inode.create(name, file_type)?;
        let parent = Arc::downgrade(self);
        let dentry = Arc::new(Dentry::new(DentryInner::new(
            name.to_owned(),
            parent,
            Some(Arc::new(VfsInode::new(new_inode))),
        )));
        let mut inner = self.inner.write();
        inner.children.insert(name.to_owned(), dentry.clone());
        Ok(dentry)
    }
    /// unlink/rmdir: remove the child entry and evict it from the dentry cache.
    pub fn remove(self: &Arc<Self>, name: &str) -> VfsResult<()> {
        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };
        let dir_inode = inode.inode();
        let dir_inode = dir_inode.as_dir_ops()?;
        drop(inner);
        dir_inode.remove(name)?;
        self.inner.write().children.remove(name);
        Ok(())
    }

    /// rename within this directory.
    pub fn rename(self: &Arc<Self>, old_name: &str, new_name: &str) -> VfsResult<()> {
        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };
        let dir_inode = inode.inode();
        let dir_inode = dir_inode.as_dir_ops()?;
        drop(inner);
        dir_inode.rename(old_name, new_name)?;
        let mut inner = self.inner.write();
        if let Some(child) = inner.children.remove(old_name) {
            child.inner.write().name = new_name.to_owned();
            inner.children.insert(new_name.to_owned(), child);
        }
        Ok(())
    }

    /// hard link: add `name` pointing at `node` in this directory.
    pub fn link(self: &Arc<Self>, name: &str, node: &Arc<VfsInode>) -> VfsResult<()> {
        let inner = self.inner.read();
        let Some(inode) = inner.inode.clone() else {
            return Err(VfsError::NotFound);
        };
        let dir_inode = inode.inode();
        let dir_inode = dir_inode.as_dir_ops()?;
        drop(inner);
        dir_inode.link(name, &node.inode())?;
        let mut inner = self.inner.write();
        let child = Dentry::new(DentryInner::new(
            name.to_owned(),
            Arc::downgrade(self),
            Some(node.clone()),
        ));
        inner.children.insert(name.to_owned(), Arc::new(child));
        Ok(())
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
    pub fn new(name: String, parent: Weak<Dentry>, inode: Option<Arc<VfsInode>>) -> Self {
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
