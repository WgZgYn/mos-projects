use crate::{DEV_DIR_DENTRY, DEV_DIR_INODE};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_driver::keyboard::CharSyncStream;
use mos_driver::{print, serial_print};
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use vfs_common::{FileType, IoContext, OpenFlags, VfsError, VfsResult};
use vfs_traits::*;

pub struct TtyInode;

lazy_static! {
    pub static ref TTY_INODE: Arc<TtyInode> = Arc::new(TtyInode);
    pub static ref TTY_DENTRY: Arc<Dentry> = DEV_DIR_DENTRY.lookup("tty").unwrap();
    pub static ref FSTDIN: Arc<File> =
        Arc::new(File::from_dentry(TTY_DENTRY.clone(), 0, OpenFlags::READ).unwrap());
    pub static ref FSTDOUT: Arc<File> =
        Arc::new(File::from_dentry(TTY_DENTRY.clone(), 0, OpenFlags::WRITE).unwrap());
}

impl FileOps for TtyInode {
    fn read(&self, ctx: &IoContext, _offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        let mut chars = CharSyncStream::default();
        let mut total = 0;
        if ctx.nonblock {
            for i in buf.iter_mut() {
                let Some(c) = chars.next_char_nonblocking() else {
                    break;
                };
                let u = c.try_into();
                if let Ok(u) = u {
                    *i = u;
                }
                total += 1;
            }
            if total == 0 {
                return Err(VfsError::Pendding);
            }
            Ok(total)
        } else {
            for i in buf.iter_mut() {
                let c = chars.next_char();
                let u = c.try_into();
                if let Ok(u) = u {
                    *i = u;
                }
            }
            Ok(buf.len())
        }
    }

    fn write(&self, _ctx: &IoContext, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        if let Ok(s) = core::str::from_utf8(buf) {
            print!("{}", s);
            serial_print!("{}", s);
            Ok(buf.len())
        } else {
            Err(VfsError::IoError)
        }
    }
}

impl Inode for TtyInode {
    fn meta_data(&self) -> &[u8] {
        b"tty0"
    }
    fn file_type(&self) -> FileType {
        FileType::CharDevice
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
}

pub const STDIN: usize = 0;
pub const STDOUT: usize = 1;
pub const STDERR: usize = 2;

#[derive(Copy, Clone, Debug)]
pub struct Stdin;
#[derive(Copy, Clone, Debug)]
pub struct Stdout;
#[derive(Copy, Clone, Debug)]
pub struct Stderr;
