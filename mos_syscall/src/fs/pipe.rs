use alloc::sync::Arc;
use log::info;
use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_fs_runtime::pipe::make_pipe;
use mos_task::current_tcb;
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use mos_vfs::vnode::VfsInode;
use vfs_common::{OpenFlags, VfsResult, vfs_result_to_isize};

pub fn raw_sys_pipe(context: &mut SyscallContext, args: [u64; 6]) {
    let fds: &mut [i32; 2] = unsafe { &mut *(args[0] as *mut [i32; 2]) };
    match sys_pipe() {
        Ok([pread, pwrite]) => {
            fds[0] = pread.0 as _;
            fds[1] = pwrite.0 as _;
            context.scratch.rax = 0;
        }
        Err(e) => {
            context.scratch.rax = vfs_result_to_isize(Err(e)) as _;
        }
    }
}

pub fn sys_pipe() -> VfsResult<[FileDesc; 2]> {
    let current_tcb = current_tcb();
    let current_pcb = &current_tcb.process;
    let inode = make_pipe();

    let fread = File::from_dentry(
        Dentry::new_anonymous(Arc::new(VfsInode::new(inode.clone()))),
        0,
        OpenFlags::READ,
    )?;

    let fwrite = File::from_dentry(
        Dentry::new_anonymous(Arc::new(VfsInode::new(inode))),
        0,
        OpenFlags::WRITE,
    )?;

    let fd0 = current_pcb.files.lock().insert(fread);
    let fd1 = current_pcb.files.lock().insert(fwrite);

    info!("pipe read: {:?}, pipe write: {:?}", fd0, fd1);
    Ok([fd0, fd1])
}

pub fn sys_pipe2(fd: &mut [i32; 2], flags: OpenFlags) -> VfsResult<()> {
    todo!()
}
