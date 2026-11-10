use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{VfsError, VfsResult, OpenFlags};

pub fn raw_sys_write(context: &mut SyscallContext, args: [u64; 6]) {
    let result = (|| {
        let file = writable_file(FileDesc(args[0] as usize))?;
        write_user(&file, args[1], args[2].min(0x7fff_f000) as usize)
    })();
    context.scratch.rax = result.unwrap_or_else(|error| error) as u64;
}

pub(super) fn writable_file(fd: FileDesc) -> Result<alloc::sync::Arc<mos_vfs::file::File>, i64> {
    let file = current_tcb().process.files.lock().get(fd).cloned().ok_or(-9i64)?;
    if !file.flags().contains(OpenFlags::WRITE) { return Err(-9); }
    Ok(file)
}

/// Stage one user page at a time. No user slice/reference survives into a
/// potentially blocking device write; a later bad page preserves prior bytes.
pub(super) fn write_user(file: &mos_vfs::file::File, address: u64, length: usize) -> Result<i64, i64> {
    let task = current_tcb();
    let mut copied = 0usize;
    let mut buffer = [0u8; 4096];
    while copied < length {
        let position = match address.checked_add(copied as u64) {
            Some(p) => p,
            None => return if copied != 0 { Ok(copied as i64) } else { Err(-14) },
        };
        let count = (length - copied).min(4096 - (position as usize & 4095));
        if let Err(error) = task.process.mem.copy_from_user(position, &mut buffer[..count]) {
            return if copied != 0 { Ok(copied as i64) } else { Err(-error.to_errno()) };
        }
        let n = match write_file(file, &buffer[..count]) {
            Ok(n) => n,
            Err(error) => return if copied != 0 { Ok(copied as i64) }
                else { Err(-vfs_common::vfs_error_to_errno(error)) },
        };
        copied += n;
        if n < count { break; }
    }
    Ok(copied as i64)
}

/// Shared by write/writev so musl stdio gets the same SIGPIPE semantics.
pub(super) fn write_file(file: &mos_vfs::file::File, buffer: &[u8]) -> VfsResult<usize> {
    let result = file.write(buffer);
    if matches!(result, Err(VfsError::BrokenPipe)) {
        crate::signal::broken_pipe_signal();
    }
    result
}
