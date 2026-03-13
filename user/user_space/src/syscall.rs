use core::arch::asm;
use core::ffi::c_char;
use vfs::UserDirEntry;

use mos_lib::syscall::Syscall;

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

fn syscall_v2(no: u64, args: [u64; 6]) -> u64 {
    let ret: u64;
    unsafe {
        asm!("syscall",
        in("rax") no,
        in("rdi") args[0],
        in("rsi") args[1],
        in("rdx") args[2],
        in("r10") args[3],
        in("r8") args[4],
        in("r9") args[5],
        lateout("rax") ret,
        clobber_abi("C")
        )
    }
    ret
}

pub fn sys_read(fd: u64, buffer: &mut [u8]) -> u64 {
    syscall(
        Syscall::Read as _,
        [fd, buffer.as_mut_ptr() as _, buffer.len() as u64],
    )
}
pub fn sys_write(fd: u64, buffer: &[u8]) -> u64 {
    syscall(
        Syscall::Write as _,
        [fd, buffer.as_ptr() as _, buffer.len() as u64],
    )
}
pub fn sys_open(path: &str, flags: u32) -> u64 {
    syscall(
        Syscall::Open as _,
        [path.as_ptr() as _, path.len() as u64, flags as u64],
    )
}
pub fn sys_close(fd: u64) -> u64 {
    syscall(Syscall::Close as _, [fd, 0, 0])
}
pub fn sys_exit(code: u64) -> ! {
    syscall(Syscall::Exit as _, [code, 0, 0]);
    unreachable!()
}

pub fn sys_rand() -> u64 {
    syscall(Syscall::Rand as _, [0, 0, 0])
}

pub fn sys_yield() -> u64 {
    syscall(Syscall::Yield as _, [0, 0, 0])
}
pub fn sys_execve(
    path: *const c_char,
    args: *const *const c_char,
    envp: *const *const c_char,
) -> isize {
    syscall(Syscall::Execve as _, [path as _, args as _, envp as _]) as _
}
pub fn sys_sleep(ms: u64) -> u64 {
    syscall(Syscall::Sleep as _, [ms, 0, 0])
}
pub fn sys_fork() -> u64 {
    syscall(Syscall::Fork as _, [0, 0, 0])
}
pub fn sys_brk(addr: u64) -> u64 {
    syscall(Syscall::Brk as _, [addr, 0, 0])
}
pub fn sys_wait(pid: u64) -> u64 {
    syscall(Syscall::Wait as _, [pid, 0, 0]) as _
}
pub fn sys_getdents(fd: u64, buf: &mut [UserDirEntry]) -> u64 {
    syscall(
        Syscall::GetDents as _,
        [fd, buf.as_mut_ptr() as _, buf.len() as u64],
    )
}
pub fn sys_chdir(fd: u64) -> u64 {
    syscall(Syscall::ChDir as _, [fd, 0, 0])
}
pub fn sys_mmap(start: u64, len: u64, prot: u32, flags: u32, fd: i32, offset: u64) -> u64 {
    syscall_v2(
        Syscall::Mmap as _,
        [start, len, prot as _, flags as _, fd as _, offset],
    )
}

pub fn sys_munmap(start: u64, len: u64) -> u64 {
    syscall(Syscall::Munmap as _, [start, len, 0])
}
