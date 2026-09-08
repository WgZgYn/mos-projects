//! readv(2)/writev(2) — musl's stdio flush uses writev, so these are needed
//! for basic printf output from a static musl binary.

use mos_abi::fs::FileDesc;
use mos_abi::io::Iovec;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, vfs_error_to_errno};

fn file_for(fd: FileDesc) -> VfsResult<alloc::sync::Arc<mos_vfs::file::File>> {
    current_tcb()
        .process
        .files()
        .lock()
        .get(fd)
        .cloned()
        .ok_or(VfsError::BadFileDesc)
}

pub fn sys_readv(fd: FileDesc, iovs: *const Iovec, cnt: usize) -> VfsResult<usize> {
    let file = file_for(fd)?;
    let mut total = 0usize;
    for i in 0..cnt {
        let iov = unsafe { iovs.add(i).read() };
        if iov.iov_base == 0 || iov.iov_len == 0 {
            continue;
        }
        let buf =
            unsafe { core::slice::from_raw_parts_mut(iov.iov_base as *mut u8, iov.iov_len as usize) };
        let n = file.read(buf)?;
        total += n;
        if n < buf.len() {
            break; // short read: the file has no more data for now
        }
    }
    Ok(total)
}

pub fn sys_writev(fd: FileDesc, iovs: *const Iovec, cnt: usize) -> VfsResult<usize> {
    let file = file_for(fd)?;
    let mut total = 0usize;
    for i in 0..cnt {
        let iov = unsafe { iovs.add(i).read() };
        if iov.iov_base == 0 || iov.iov_len == 0 {
            continue;
        }
        let buf = unsafe { core::slice::from_raw_parts(iov.iov_base as *const u8, iov.iov_len as usize) };
        let n = file.write(buf)?;
        total += n;
        if n < buf.len() {
            break;
        }
    }
    Ok(total)
}

pub fn raw_sys_readv(context: &mut SyscallContext, args: [u64; 6]) {
    let fd = FileDesc(args[0] as usize);
    let iovs = args[1] as *const Iovec;
    let cnt = args[2] as usize;
    context.scratch.rax = match sys_readv(fd, iovs, cnt) {
        Ok(n) => n as u64,
        Err(e) => (-vfs_error_to_errno(e)) as u64,
    };
}

pub fn raw_sys_writev(context: &mut SyscallContext, args: [u64; 6]) {
    let fd = FileDesc(args[0] as usize);
    let iovs = args[1] as *const Iovec;
    let cnt = args[2] as usize;
    context.scratch.rax = match sys_writev(fd, iovs, cnt) {
        Ok(n) => n as u64,
        Err(e) => (-vfs_error_to_errno(e)) as u64,
    };
}
