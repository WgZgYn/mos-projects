use mos_abi::fs::{FileDesc, SEEK_CUR, SEEK_END, SEEK_SET};
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, vfs_error_to_errno};

pub fn raw_sys_lseek(context: &mut SyscallContext, args: [u64; 6]) {
    let fd = FileDesc(args[0] as usize);
    let offset = args[1] as i64;
    let whence = args[2] as u32;
    context.scratch.rax = match sys_lseek(fd, offset, whence) {
        Ok(pos) => pos as u64,
        Err(e) => (-vfs_error_to_errno(e)) as u64,
    };
}

pub fn sys_lseek(fd: FileDesc, offset: i64, whence: u32) -> VfsResult<i64> {
    let current_tcb = current_tcb();
    let files = current_tcb.process.files().lock();
    let file = files.get(fd).cloned().ok_or(VfsError::BadFileDesc)?;

    let base = match whence {
        SEEK_SET => 0,
        SEEK_CUR => file.get_offset() as i64,
        SEEK_END => file.dentry().file_size()? as i64,
        _ => return Err(VfsError::FileNameIllegal), // EINVAL
    };
    let new_off = base
        .checked_add(offset)
        .ok_or(VfsError::FileNameIllegal)?;
    if new_off < 0 {
        return Err(VfsError::FileNameIllegal);
    }

    file.set_offset(new_off as u64);
    Ok(new_off)
}
