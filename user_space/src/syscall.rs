use core::arch::asm;
use vfs::UserDirEntry;

fn syscall(no: u64, args: [u64; 3]) -> u64 {
    let ret: u64;
    unsafe {
        asm!("syscall",
        in("rax") no,
        in("rdi") args[0],
        in("rsi") args[1],
        in("rdx") args[2],
        lateout("rax") ret,
        clobber_abi("C")
        )
    }
    ret
}

pub fn sys_read(fd: u64, buffer: &mut [u8]) -> u64 {
    syscall(0, [fd, buffer.as_mut_ptr() as _, buffer.len() as u64])
}
pub fn sys_write(fd: u64, buffer: &[u8]) -> u64 {
    syscall(1, [fd, buffer.as_ptr() as _, buffer.len() as u64])
}
pub fn sys_open(path: &str, flags: u32) -> u64 {
    syscall(2, [path.as_ptr() as _, path.len() as u64, flags as u64])
}
pub fn sys_close(fd: u64) -> u64 {
    syscall(3, [fd, 0, 0])
}
pub fn sys_exit(code: u64) -> u64 {
    syscall(60, [code, 0, 0])
}

pub fn sys_rand() -> u64 {
    syscall(7, [0, 0, 0])
}

pub fn sys_yield() -> u64 {
    syscall(24, [0, 0, 0])
}
pub fn sys_exec(path: &str, priority: u8) -> u64 {
    syscall(59, [path.as_ptr() as _, path.len() as _, priority as _])
}
pub fn sys_sleep(ms: u64) -> u64 {
    syscall(62, [ms, 0, 0])
}
pub fn sys_fork() -> u64 {
    syscall(57, [0, 0, 0])
}
pub fn sys_brk(addr: u64) -> u64 {
    syscall(12, [addr, 0, 0])
}
pub fn sys_wait(pid: u64) -> u64 {
    syscall(61, [pid, 0, 0]) as _
}
pub fn sys_getdents(fd: u64, buf: &mut [UserDirEntry]) -> u64 {
    syscall(78, [fd, buf.as_mut_ptr() as _, buf.len() as u64])
}
pub fn sys_chdir(fd: u64) -> u64 {
    syscall(80, [fd, 0, 0])
}
