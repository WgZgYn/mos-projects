use mos_abi::ffi::c_str_to_str;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{FileType, VfsResult, vfs_result_to_isize};

pub fn raw_sys_mkfifo(context: &mut SyscallContext, args: [u64; 6]) {
    let path = unsafe { c_str_to_str(args[0] as _) };
    let Ok(path) = path else {
        context.scratch.rax = u64::MAX;
        return;
    };
    context.scratch.rax = vfs_result_to_isize(sys_mkfifo(path).map(|_| 0)) as _;
}

pub fn sys_mkfifo(path: &str) -> VfsResult<()> {
    let current_tcb = current_tcb();
    let cwd = current_tcb.process.work_dir().lock().clone();
    mos_vfs::vfs_create(mos_vfs::vfs_root_dentry(), cwd, path, FileType::Fifo)?;
    Ok(())
}
