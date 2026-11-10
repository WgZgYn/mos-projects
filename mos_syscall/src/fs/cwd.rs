//! Linux pathname chdir, descriptor fchdir (in fs.rs), and raw getcwd ABI.
use alloc::{string::String, vec::Vec};
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::OpenFlags;

pub fn raw_chdir(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = (|| -> Result<i64, i64> {
        let task = current_tcb();
        let mut bytes = Vec::new();
        // Read only through the terminating NUL, including at page boundaries.
        for index in 0..4096u64 {
            let addr = args[0].checked_add(index).ok_or(-14i64)?;
            let mut byte = [0];
            task.process.mem.copy_from_user(addr, &mut byte).map_err(|e| -e.to_errno())?;
            if byte[0] == 0 {
                if bytes.is_empty() { return Err(-2); }
                let path = core::str::from_utf8(&bytes).map_err(|_| -22i64)?;
                let cwd = task.process.work_dir.lock().clone();
                let file = mos_fs_runtime::open_with_dentry(mos_vfs::vfs_root_dentry(), cwd,
                    path, OpenFlags::READ | OpenFlags::DIRECTORY)
                    .map_err(|e| -vfs_common::vfs_error_to_errno(e))?;
                if file.inode().file_type() != vfs_common::FileType::Directory {
                    return Err(-20);
                }
                *task.process.work_dir.lock() = file.dentry().clone();
                return Ok(0);
            }
            bytes.push(byte[0]);
        }
        Err(-36) // ENAMETOOLONG
    })().unwrap_or_else(|e| e) as u64;
}

pub fn raw_getcwd(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = (|| -> Result<i64, i64> {
        if args[1] == 0 { return Err(-34); }
        let task = current_tcb();
        let mut current = task.process.work_dir.lock().clone();
        let root = mos_vfs::vfs_root_dentry();
        let mut components = Vec::new();
        let mut length = 1usize;
        while !alloc::sync::Arc::ptr_eq(&current, &root) {
            let (name, parent) = current.path_location();
            let parent = parent.ok_or(-2i64)?;
            length += name.len() + 1;
            if length > 4096 { return Err(-36); }
            components.push(name);
            current = parent;
        }
        let mut path = String::from("/");
        for name in components.iter().rev() {
            if path.len() > 1 { path.push('/'); }
            path.push_str(name);
        }
        path.push('\0');
        if args[1] < path.len() as u64 { return Err(-34); } // ERANGE
        task.process.mem.validate_user_write(args[0], path.len()).map_err(|e| -e.to_errno())?;
        let result = task.process.mem.copy_to_user(args[0], path.as_bytes());
        mos_platform::smp::tlb_shootdown();
        result.map_err(|e| -e.to_errno())?;
        Ok(path.len() as i64) // raw syscall returns byte count, including NUL
    })().unwrap_or_else(|e| e) as u64;
}
