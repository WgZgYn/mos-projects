use crate::dentry::Dentry;
use alloc::sync::Arc;
use mos_abi::io::{IOError, IOResult, Read};
use spin::Mutex;
use vfs_common::VfsError::NotFound;
use vfs_common::{FileType, IoContext, OpenFlags, VfsError, VfsResult};
use vfs_traits::{DirEntry, DirOps, FileOps, Inode, StreamOps};

pub enum Ops {
    Dir(Arc<dyn DirOps>),
    File(Arc<dyn FileOps>),
    Fifo(Arc<dyn StreamOps>),
}

pub struct File {
    inode: Arc<dyn Inode>,
    dentry: Arc<Dentry>,
    offset: Mutex<u64>,
    flags: OpenFlags,
}

impl File {
    pub fn inode(&self) -> &Arc<dyn Inode> {
        &self.inode
    }
    pub fn flags(&self) -> OpenFlags {
        self.flags
    }
    pub fn dentry(&self) -> &Arc<Dentry> {
        &self.dentry
    }
    pub fn from_dentry(dentry: Arc<Dentry>, offset: u64, flags: OpenFlags) -> VfsResult<Self> {
        Self::from_inode(
            dentry.inode().ok_or(NotFound)?.inode(),
            dentry,
            offset,
            flags,
        )
    }
    pub fn from_inode(
        inode: Arc<dyn Inode>,
        dentry: Arc<Dentry>,
        offset: u64,
        flags: OpenFlags,
    ) -> VfsResult<Self> {
        inode.on_open(flags)?;
        Ok(Self {
            inode,
            dentry,
            offset: Mutex::new(offset),
            flags,
        })
    }
    pub fn read(&self, buf: &mut [u8]) -> VfsResult<usize> {
        if !self.flags.contains(OpenFlags::READ) {
            return Err(VfsError::PermissionDenied);
        }
        match self.inode.file_type() {
            FileType::Regular | FileType::BlockDevice => {
                let mut offset = self.offset.lock();
                let file = self.inode.as_file_ops()?;
                let n = file.read_at(*offset as _, buf)?;
                *offset += n as u64;
                Ok(n)
            }
            FileType::CharDevice | FileType::Fifo => self.inode.as_stream_ops()?.read(
                &IoContext::new(self.flags.contains(OpenFlags::NONBLOCK)),
                buf,
            ),
            _ => Err(VfsError::NotSupported),
        }
    }
    pub fn write(&self, buf: &[u8]) -> VfsResult<usize> {
        if !self.flags.contains(OpenFlags::WRITE) {
            return Err(VfsError::PermissionDenied);
        }
        match self.inode.file_type() {
            FileType::Regular | FileType::BlockDevice => {
                let mut offset = self.offset.lock();
                let file = self.inode.as_file_ops()?;
                let n = file.write_at(*offset as _, buf)?;
                *offset += n as u64;
                Ok(n)
            }
            FileType::CharDevice | FileType::Fifo => self.inode.as_stream_ops()?.write(
                &IoContext::new(self.flags.contains(OpenFlags::NONBLOCK)),
                buf,
            ),
            _ => Err(VfsError::NotSupported),
        }
    }
    pub fn set_offset(&mut self, offset: u64) {
        *self.offset.lock() = offset;
    }
    /// this need check file type early at vfs_open
    pub fn is_dir(&self) -> bool {
        self.flags.contains(OpenFlags::DIRECTORY)
    }
    pub fn read_dir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        self.inode.as_dir_ops()?.readdir(offset)
    }
}

impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        File::read(self, buf).map_err(|_| IOError::UNKNOWN)
    }
}

impl Read for &File {
    fn read(&mut self, buf: &mut [u8]) -> IOResult<usize> {
        File::read(self, buf).map_err(|_| IOError::UNKNOWN)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = self.inode.on_close(self.flags);
    }
}
