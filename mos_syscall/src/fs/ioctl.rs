//! ioctl(2) — device control. TCGETS/TIOCGWINSZ on the tty make isatty(3)
//! succeed so busybox can detect a terminal (line editing, columnar ls).

use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, vfs_result_to_isize};

pub fn sys_ioctl(fd: FileDesc, cmd: u32, arg: u64) -> VfsResult<usize> {
    let tcb = current_tcb();
    let files = tcb.process.files().lock();
    let file = files.get(fd).ok_or(VfsError::BadFileDesc)?;
    file.inode().ioctl(cmd, arg)
}

pub fn raw_sys_ioctl(context: &mut SyscallContext, args: [u64; 6]) {
    let ret = sys_ioctl(FileDesc(args[0] as usize), args[1] as u32, args[2]);
    context.scratch.rax = match ret {
        Ok(n) => n as u64,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}
