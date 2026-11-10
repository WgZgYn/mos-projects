//! ioctl(2) — device control. TCGETS/TIOCGWINSZ on the tty make isatty(3)
//! succeed so busybox can detect a terminal (line editing, columnar ls).

use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, vfs_result_to_isize};

pub fn sys_ioctl(fd: FileDesc, cmd: u32, arg: u64) -> VfsResult<usize> {
    let tcb = current_tcb();
    // A device ioctl may inspect task/session state or fault in user pages.
    // Retain the open file, not the file-table lock, across that work.
    let file = tcb.process.files().lock().get(fd).cloned()
        .ok_or(VfsError::BadFileDesc)?;
    use mos_abi::fs::{TCGETS, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGSID,
        TIOCGWINSZ, TIOCSPGRP, Termios};
    let (input_len, output_len) = match cmd {
        TCSETS | TCSETSW | TCSETSF => (Termios::BYTE_LEN, 0),
        TCGETS => (0, Termios::BYTE_LEN),
        TIOCSPGRP => (4, 0),
        TIOCGPGRP | TIOCGSID => (0, 4),
        TIOCGWINSZ => (0, 8),
        _ => (0, 0),
    };
    if (input_len != 0 || output_len != 0) && arg == 0 {
        return Err(VfsError::BadAddress);
    }
    let mut input = [0u8; Termios::BYTE_LEN];
    if input_len != 0 {
        tcb.process.mem.copy_from_user(arg, &mut input[..input_len])
            .map_err(|_| VfsError::BadAddress)?;
    }
    let mut output = [0u8; Termios::BYTE_LEN];
    let result = file.inode().ioctl(cmd, arg, &input[..input_len], &mut output[..output_len])?;
    if output_len != 0 {
        let copied = tcb.process.mem.copy_to_user(arg, &output[..output_len]);
        // copy_to_user may resolve CoW before a later page fails.
        mos_platform::smp::tlb_shootdown();
        copied.map_err(|_| VfsError::BadAddress)?;
    }
    Ok(result)
}

pub fn raw_sys_ioctl(context: &mut SyscallContext, args: [u64; 6]) {
    let ret = sys_ioctl(FileDesc(args[0] as usize), args[1] as u32, args[2]);
    context.scratch.rax = match ret {
        Ok(n) => n as u64,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}
