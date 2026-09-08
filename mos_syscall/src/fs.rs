mod close;
mod iovec;
mod lseek;
mod mkfifo;
mod open;
mod pipe;
mod read;
mod stat;
mod write;

use alloc::sync::Arc;
use alloc::vec::Vec;
use log::info;
use mos_abi::fs::{DIRENT64_HEADER, FileDesc};
use mos_lib::align::align_up;
use mos_task::current_tcb;
use vfs_common::{FileType, VfsError, VfsResult};

pub use close::raw_sys_close;
pub use iovec::{raw_sys_readv, raw_sys_writev};
pub use lseek::raw_sys_lseek;
pub use mkfifo::raw_sys_mkfifo;
pub use mkfifo::sys_mkfifo;
pub use open::raw_sys_open;
pub use pipe::raw_sys_pipe;
pub use read::raw_sys_read;
pub use stat::{raw_sys_fstat, raw_sys_stat, stat_from_file};
pub use write::raw_sys_write;

/// getdents64(2): pack Linux dirent64 records into the caller's buffer.
/// The user side (musl readdir / user_lib) parses this layout directly.
pub fn sys_getdents(fd: FileDesc, buf: &mut [u8]) -> VfsResult<usize> {
    let current_tcb = current_tcb();
    let file = current_tcb.process.files().lock().get(fd).cloned();
    if file.is_none() {
        return Err(VfsError::NotOpen);
    }
    let file = file.unwrap();

    let mut pos = 0usize;
    let mut idx = 0usize;
    while let Some(entry) = file.read_dir(idx)? {
        let name = entry.name();
        let name_len = name.len();
        // header + NUL-terminated name, padded to 8-byte alignment
        let reclen = align_up((DIRENT64_HEADER + 1 + name_len + 1) as u64, 8) as usize;
        if pos + reclen > buf.len() {
            break;
        }

        let rec = &mut buf[pos..pos + reclen];
        let ino = Arc::as_ptr(&entry.node()) as *const () as u64;
        rec[0..8].copy_from_slice(&ino.to_ne_bytes());
        rec[8..16].copy_from_slice(&((idx + 1) as i64).to_ne_bytes());
        rec[16..18].copy_from_slice(&(reclen as u16).to_ne_bytes());
        rec[18] = super::fs::stat::file_type_to_dt(entry.file_type());
        rec[19..19 + name_len].copy_from_slice(name.as_bytes());
        rec[19 + name_len] = 0; // NUL terminator
        // padding bytes stay zeroed by the caller's read_dir contract

        pos += reclen;
        idx += 1;
    }
    Ok(pos)
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
