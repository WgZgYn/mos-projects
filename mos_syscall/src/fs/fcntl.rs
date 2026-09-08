//! fcntl(2)/dup3(2) — descriptor manipulation busybox uses for redirection
//! (F_DUPFD in xmove_fd) and the generic dup variants.

use mos_abi::fs::{FileDesc, O_APPEND, O_NONBLOCK, O_RDONLY, O_RDWR, O_WRONLY};
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{OpenFlags, VfsError, VfsResult, vfs_result_to_isize};

// Linux fcntl commands
const F_DUPFD: u32 = 0;
const F_GETFD: u32 = 1;
const F_SETFD: u32 = 2;
const F_GETFL: u32 = 3;
const F_SETFL: u32 = 4;

fn open_flags_to_linux(f: OpenFlags) -> u32 {
    let mut o = match (f.contains(OpenFlags::READ), f.contains(OpenFlags::WRITE)) {
        (true, true) => O_RDWR,
        (false, true) => O_WRONLY,
        _ => O_RDONLY,
    };
    if f.contains(OpenFlags::APPEND) {
        o |= O_APPEND;
    }
    if f.contains(OpenFlags::NONBLOCK) {
        o |= O_NONBLOCK;
    }
    o
}

pub fn sys_fcntl(fd: FileDesc, cmd: u32, arg: u64) -> VfsResult<u64> {
    let tcb = current_tcb();
    let mut files = tcb.process.files().lock();
    match cmd {
        F_DUPFD => {
            let new = files.dup_from(fd, arg as usize).ok_or(VfsError::BadFileDesc)?;
            Ok(new.0 as u64)
        }
        F_GETFD => Ok(0), // FD_CLOEXEC not tracked (exec does not close-on-exec yet)
        F_SETFD => Ok(0),
        F_GETFL => {
            let f = files.get(fd).ok_or(VfsError::BadFileDesc)?;
            Ok(open_flags_to_linux(f.flags()) as u64)
        }
        F_SETFL => {
            // accepted but ignored: File flags are fixed at open time for now
            let _ = files.get(fd).ok_or(VfsError::BadFileDesc)?;
            Ok(0)
        }
        _ => Err(VfsError::NotSupported),
    }
}

pub fn sys_dup3(src: FileDesc, dst: FileDesc, _flags: u32) -> VfsResult<FileDesc> {
    let tcb = current_tcb();
    let mut files = tcb.process.files().lock();
    if src == dst {
        return Ok(src);
    }
    if files.dup_to(src, dst) {
        Ok(dst)
    } else {
        Err(VfsError::BadFileDesc)
    }
}

pub fn raw_sys_fcntl(context: &mut SyscallContext, args: [u64; 6]) {
    let ret = sys_fcntl(FileDesc(args[0] as usize), args[1] as u32, args[2]);
    context.scratch.rax = match ret {
        Ok(n) => n,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_dup3(context: &mut SyscallContext, args: [u64; 6]) {
    let ret = sys_dup3(
        FileDesc(args[0] as usize),
        FileDesc(args[1] as usize),
        args[2] as u32,
    );
    context.scratch.rax = match ret {
        Ok(fd) => fd.0 as u64,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}
