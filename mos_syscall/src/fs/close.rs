use log::info;
use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;

pub fn raw_sys_close(context: &mut SyscallContext, args: [u64; 6]) {
    let fd = args[0];
    let fd = FileDesc(fd as _);
    if sys_close(fd) {
        context.scratch.rax = 0;
    } else {
        context.scratch.rax = u64::MAX;
    }
}

pub fn sys_close(fd: FileDesc) -> bool {
    info!("sys_close: {:?}", fd);
    let current_tcb = current_tcb();
    current_tcb.process.files().lock().remove(fd)
}
