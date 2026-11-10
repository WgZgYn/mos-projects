use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{FileType, OpenFlags, VfsError};

pub(super) fn readable_file(fd: FileDesc) -> Result<alloc::sync::Arc<mos_vfs::file::File>, i64> {
    let file = current_tcb().process.files.lock().get(fd).cloned().ok_or(-9i64)?;
    if !file.flags().contains(OpenFlags::READ) { return Err(-9); }
    Ok(file)
}

pub(super) fn read_user(file: &mos_vfs::file::File, address: u64, length: usize,
    nonblock: bool) -> Result<i64, i64> {
    let task = current_tcb();
    let mut total = 0;
    while total < length {
        let position = match address.checked_add(total as u64) {
            Some(p) => p,
            None => return if total != 0 { Ok(total as i64) } else { Err(-14) },
        };
        let count = (length - total).min(4096 - (position as usize & 4095));
        if let Err(e) = task.process.mem.validate_user_write(position, count) {
            return if total != 0 { Ok(total as i64) } else { Err(-e.to_errno()) };
        }
        let mut copy_error = -14;
        let result = file.read_to(count, nonblock || total != 0, &mut |bytes| {
            let result = task.process.mem.copy_to_user(position, bytes);
            // Copy may resolve COW even on a subsequently failed writeback.
            mos_platform::smp::tlb_shootdown();
            result.map_err(|e| { copy_error = -e.to_errno(); VfsError::BadAddress })
        });
        let n = match result {
            Ok(n) => n,
            Err(_) if total != 0 => return Ok(total as i64),
            Err(VfsError::BadAddress) => return Err(copy_error),
            Err(e) => return Err(-vfs_common::vfs_error_to_errno(e)),
        };
        total += n;
        // Streams return available data rather than waiting to fill more pages.
        if n < count || file.inode().file_type() != FileType::Regular { break; }
    }
    Ok(total as i64)
}

pub fn raw_sys_read(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = (|| {
        let file = readable_file(FileDesc(args[0] as usize))?;
        read_user(&file, args[1], args[2].min(0x7fff_f000) as usize, false)
    })().unwrap_or_else(|error| error) as u64;
}
