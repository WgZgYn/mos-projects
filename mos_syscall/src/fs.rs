mod close;
mod mkfifo;
mod open;
mod pipe;
mod read;
mod write;

use alloc::vec::Vec;
use log::info;
use mos_abi::fs::FileDesc;
use mos_task::current_tcb;
use vfs_common::{FileType, UserDirEntry, VfsError, VfsResult};

pub use close::raw_sys_close;
pub use mkfifo::raw_sys_mkfifo;
pub use mkfifo::sys_mkfifo;
pub use open::raw_sys_open;
pub use pipe::raw_sys_pipe;
pub use read::raw_sys_read;
pub use write::raw_sys_write;

pub fn sys_getdents(fd: FileDesc, buf: &mut [UserDirEntry]) -> VfsResult<usize> {
    let current_tcb = current_tcb();
    let file = current_tcb.process.files().lock().get(fd).cloned();
    if let Some(file) = file {
        let mut i = 0;
        while let Some(entry) = file.read_dir(i)?
            && i < buf.len()
        {
            let name = entry.name();
            let mut buffer = [0; 32];
            let n = name.len().min(32);
            buffer[..n].copy_from_slice(&name.as_bytes()[..n]);
            buf[i] = UserDirEntry::new(buffer, entry.file_type(), entry.node().file_size());
            i += 1;
        }
        Ok(i)
    } else {
        Err(VfsError::NotOpen)
    }
}

pub fn sys_chdir(fd: FileDesc) -> VfsResult<()> {
    let current_tcb = current_tcb();
    let file = current_tcb.process.files().lock().get(fd).cloned();
    if let Some(file) = file {
        let mut p = file.dentry().clone();
        let mut v = Vec::new();
        loop {
            v.push(p.name());
            if let Some(np) = p.parent() {
                p = np;
            } else {
                break;
            }
        }
        info!("chdir to {:?}", v);

        let inode = file.inode();
        if inode.file_type() == FileType::Directory {
            *current_tcb.process.work_dir().lock() = file.dentry().clone();
            Ok(())
        } else {
            Err(VfsError::NotDir)
        }
    } else {
        Err(VfsError::NotOpen)
    }
}
