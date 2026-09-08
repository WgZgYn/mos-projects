use mos_abi::ffi::c_str_to_str;
use mos_abi::fs::{
    FileDesc, O_APPEND, O_CREAT, O_DIRECTORY, O_NONBLOCK, O_RDONLY, O_RDWR, O_TRUNC, O_WRONLY,
};
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{OpenFlags, VfsError, VfsResult, vfs_result_to_isize};

/// Translate Linux O_* bits (as passed by musl/glibc) to the kernel's
/// internal OpenFlags.
pub fn linux_to_open_flags(flags: u32) -> OpenFlags {
    let mut f = OpenFlags::empty();
    match flags & 3 {
        O_RDONLY => f.insert(OpenFlags::READ),
        O_WRONLY => f.insert(OpenFlags::WRITE),
        _ => f.insert(OpenFlags::READ_WRITE),
    }
    if flags & O_CREAT != 0 {
        f.insert(OpenFlags::CREATE);
    }
    if flags & O_TRUNC != 0 {
        f.insert(OpenFlags::TRUNC);
    }
    if flags & O_APPEND != 0 {
        f.insert(OpenFlags::APPEND);
    }
    if flags & O_NONBLOCK != 0 {
        f.insert(OpenFlags::NONBLOCK);
    }
    if flags & O_DIRECTORY != 0 {
        f.insert(OpenFlags::DIRECTORY);
    }
    // O_EXCL is meaningless without O_CREAT; the VFS create path rejects
    // already-existing names anyway
    f
}

pub fn raw_sys_open(context: &mut SyscallContext, args: [u64; 6]) {
    let arg0_user_path_ptr = args[0];
    let arg1_open_flags = args[1];

    let path = unsafe { c_str_to_str(arg0_user_path_ptr as _) };
    let Ok(path) = path else {
        context.scratch.rax = vfs_result_to_isize(Err(VfsError::FileNameIllegal)) as _;
        return;
    };
    let flags = linux_to_open_flags(arg1_open_flags as _);
    let ret = sys_open(path, flags);
    context.scratch.rax = vfs_result_to_isize(ret.map(|fd| fd.0)) as _;
}

pub fn sys_open(path: &str, flags: OpenFlags) -> VfsResult<FileDesc> {
    let current_tcb = current_tcb();
    let work_dir = current_tcb.process.work_dir().lock().clone();
    let file = mos_fs_runtime::open_with_dentry(mos_vfs::vfs_root_dentry(), work_dir, path, flags)?;
    let fd = current_tcb.process.files().lock().insert(file);
    Ok(fd)
}
