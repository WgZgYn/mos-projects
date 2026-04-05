use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, vfs_result_to_isize};

pub fn raw_sys_write(context: &mut SyscallContext, args: [u64; 6]) {
    let arg0_fd = args[0];
    let arg1_user_src_addr = args[1];
    let arg2_user_src_len = args[2];

    let fd = FileDesc(arg0_fd as _);
    let user_buffer: &mut [u8] =
        unsafe { core::slice::from_raw_parts_mut(arg1_user_src_addr as _, arg2_user_src_len as _) };

    let ret = sys_write(fd, user_buffer);
    context.scratch.rax = vfs_result_to_isize(ret) as _;
}

pub fn sys_write(fd: FileDesc, buffer: &[u8]) -> VfsResult<usize> {
    let current_tcb = current_tcb();
    let current_pcb = &current_tcb.process;
    let file = current_pcb.files.lock().get(fd).cloned();
    file.ok_or(VfsError::NotOpen)?.write(buffer)
}
