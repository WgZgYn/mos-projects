use crate::file::File;
use crate::fs::{DirItem, Error, FileType, has_same_name};
use crate::fs_manager::RamFsManager;
use crate::utils::FsTransaction;
use kernel_utils::trace;
use vfs::{FileSystem, OpenFlag};

impl<'a> FileSystem for RamFsManager<'a> {
    type Error = Error;
    type FileType = FileType;
    type File = File;

    // current open is read_write at 0
    fn open(&self, path: &str, _open_flag: OpenFlag) -> Result<Self::File, Self::Error> {
        let (parent_inode_idx, name) = self.lock().resolve_path(path)?;
        let fs = self.lock();
        let inode = &fs[parent_inode_idx];
        let len = inode.size as usize / size_of::<DirItem>();
        inode.page_1[0]
            .to_option()
            .and_then(|f| {
                fs[f]
                    .as_dir_items(len)
                    .iter()
                    .find(|item| has_same_name(&item.name, name))
                    .map(|item| File::from_fd(item.index))
            })
            .ok_or(Error::NoSuchFile)
    }

    fn create(&self, path: &str, file_type: Self::FileType) -> Result<Self::File, Self::Error> {
        trace!("FS CREATE {}", path);

        // resolve the path
        let (parent_inode_idx, name) = self.lock().resolve_path(path)?;

        // create a transaction
        let mut trans = FsTransaction::new(self);

        let mut fs = self.lock();

        // allocate inode
        let inode_idx = fs.new_inode(name, file_type, parent_inode_idx)?;

        // add it to the parent dir
        fs.add_dir_entry(parent_inode_idx, inode_idx)?;

        // trans is success
        trans.success();
        Ok(File::from_fd(inode_idx))
    }

    fn remove(&self, path: &str) -> Result<(), Self::Error> {
        let (parent_inode_idx, name) = self.lock().resolve_path(path)?;
        let inode = &mut self.lock()[parent_inode_idx];
        let len = inode.size as usize / size_of::<DirItem>();
        inode.page_1[0]
            .to_option()
            .map(|f| {
                let i = self.lock()[f]
                    .as_dir_items(len)
                    .iter()
                    .enumerate()
                    .find(|(_, item)| has_same_name(&item.name, name))
                    .map(|(i, _)| i);
                if let Some(i) = i {
                    self.lock()[f].as_dir_items_mut(len).swap(i, len);
                    inode.size -= size_of::<DirItem>() as u64;
                }
            })
            .ok_or(Error::NoSuchFile)
    }

    fn close(&self, _file: Self::File) -> Result<(), Self::Error> {
        Ok(())
    }
}
