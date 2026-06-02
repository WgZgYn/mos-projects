use core::arch::asm;
use core::ffi::{CStr, c_char};

use mos_abi::syscall::Syscall;
use vfs_common::UserDirEntry;

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

pub fn sys_read(fd: u64, buffer: &mut [u8]) -> isize {
    syscall(
        Syscall::Read as _,
        [fd, buffer.as_mut_ptr() as _, buffer.len() as u64],
    ) as _
}
pub fn sys_write(fd: u64, buffer: &[u8]) -> isize {
    syscall(
        Syscall::Write as _,
        [fd, buffer.as_ptr() as _, buffer.len() as u64],
    ) as _
}
pub fn sys_open(path: *const c_char, flags: u32) -> isize {
    syscall(Syscall::Open as _, [path as _, flags as u64, 0]) as _
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
pub fn sys_getcpu() -> u64 {
    syscall(Syscall::GetCpu as _, [0, 0, 0])
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
pub fn sys_pipe(addr: &mut [i32; 2]) -> i32 {
    syscall(Syscall::Pipe as _, [addr.as_mut_ptr() as _, 0, 0]) as _
}
pub fn sys_mkfifo(path: &CStr) -> isize {
    syscall(Syscall::Mkfifo as _, [path.as_ptr() as _, 0, 0]) as _
}
pub fn sys_dup(fd: u32) -> isize {
    syscall(Syscall::Dup as _, [fd as _, 0, 0]) as _
}
pub fn sys_dup2(src_fd: u32, dst_fd: u32) -> isize {
    syscall(Syscall::DupTo as _, [src_fd as _, dst_fd as _, 0]) as _
}
pub fn sys_create_thread(entry: u64, stack: u64) -> u64 {
    syscall(Syscall::CreateThread as _, [entry as _, stack as _, 0]) as _
}
pub fn sys_sem_create(init: u64) -> u64 {
    syscall(Syscall::CreateSemaphore as _, [init, 0, 0])
}
pub fn sys_sem_signal(sem_id: u64) {
    syscall(Syscall::SignalSemaphore as _, [sem_id, 0, 0]);
}
pub fn sys_sem_wait(sem_id: u64) {
    syscall(Syscall::WaitSemaphore as _, [sem_id, 0, 0]);
}
pub fn sys_clock_ticks() -> u64 {
    syscall(Syscall::ClockTicks as _, [0, 0, 0])
}
pub fn sys_clock_freq() -> u64 {
    syscall(Syscall::ClockFreq as _, [0, 0, 0])
}
pub fn sys_create_window(x: u64, y: u64, w: u64, h: u64, out: *mut u64) -> u64 {
    syscall_v2(Syscall::CreateWindow as _, [x, y, w, h, out as u64, 0])
}
pub fn sys_window_mark_dirty(win_id: u64, x: u64, y: u64, w: u64, h: u64) {
    syscall_v2(Syscall::WindowMarkDirty as _, [win_id, x, y, w, h, 0]);
}
pub fn sys_window_sync() {
    syscall(Syscall::WindowSync as _, [0, 0, 0]);
}
pub fn sys_destroy_window(win_id: u64) {
    syscall(Syscall::DestroyWindow as _, [win_id, 0, 0]);
}
