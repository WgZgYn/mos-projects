//! Checked descriptor snapshots shared by readv/writev.
use mos_abi::fs::FileDesc;
use mos_abi::io::Iovec;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;

fn import_vectors(address: u64, count: usize) -> Result<alloc::vec::Vec<Iovec>, i64> {
    if count > 1024 { return Err(-22); }
    let task = current_tcb();
    let mut vectors = alloc::vec::Vec::new();
    vectors.try_reserve_exact(count).map_err(|_| -12i64)?;
    let mut length = 0u64;
    for index in 0..count {
        let pointer = address.checked_add(index as u64 * 16).ok_or(-14i64)?;
        let mut bytes = [0u8; 16];
        task.process.mem.copy_from_user(pointer, &mut bytes).map_err(|e| -e.to_errno())?;
        let base = u64::from_ne_bytes(bytes[..8].try_into().unwrap());
        let len = u64::from_ne_bytes(bytes[8..].try_into().unwrap());
        length = length.checked_add(len).filter(|n| *n <= i64::MAX as u64).ok_or(-22i64)?;
        vectors.push(Iovec { iov_base: base, iov_len: len });
    }
    Ok(vectors)
}

fn transfer(fd: FileDesc, address: u64, count: usize, read: bool) -> Result<i64, i64> {
    let file = if read { super::read::readable_file(fd)? } else { super::write::writable_file(fd)? };
    // Snapshot the whole descriptor array before performing any device I/O.
    let vectors = import_vectors(address, count)?;
    let mut total = 0usize;
    for iov in vectors {
        let count = (iov.iov_len as usize).min(0x7fff_f000 - total);
        if count == 0 { continue; }
        let result = if read {
            // Once any bytes are read, a later empty pipe must return a partial
            // result instead of blocking a second time for another vector.
            super::read::read_user(&file, iov.iov_base, count, total != 0)
        } else { super::write::write_user(&file, iov.iov_base, count) };
        let n = match result {
            Ok(n) => n as usize,
            Err(_) if total != 0 => return Ok(total as i64),
            Err(e) => return Err(e),
        };
        total += n;
        if n < count || total == 0x7fff_f000 { break; }
    }
    Ok(total as i64)
}

pub fn raw_sys_readv(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = transfer(FileDesc(args[0] as usize), args[1], args[2] as usize, true)
        .unwrap_or_else(|error| error) as u64;
}
pub fn raw_sys_writev(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = transfer(FileDesc(args[0] as usize), args[1], args[2] as usize, false)
        .unwrap_or_else(|error| error) as u64;
}
