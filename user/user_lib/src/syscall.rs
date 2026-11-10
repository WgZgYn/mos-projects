use core::arch::asm;
use core::ffi::{CStr, c_char};

use mos_abi::fs::Stat;
use mos_abi::syscall::Syscall;
use mos_abi::time::{Timespec, Timeval, Timezone};

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

pub fn sys_rt_sigprocmask(how: i32, set: u64, oldset: u64) -> i64 {
    syscall_v2(Syscall::RtSigprocmask as _, [how as u64, set, oldset, 8, 0, 0]) as i64
}

pub fn sys_rt_sigaction(signal: i32, act: u64, oldact: u64) -> i64 {
    syscall_v2(Syscall::RtSigaction as _, [signal as u64, act, oldact, 8, 0, 0]) as i64
}

pub fn sys_rt_sigpending(set: u64) -> i64 {
    syscall_v2(Syscall::RtSigpending as _, [set, 8, 0, 0, 0, 0]) as i64
}

pub fn sys_kill(id: i32, signal: i32, thread_directed: bool) -> i64 {
    let number = if thread_directed { Syscall::Tkill } else { Syscall::Kill };
    syscall(number as u64, [id as u64, signal as u64, 0]) as i64
}

pub fn sys_gettid() -> u64 { syscall(Syscall::GetTid as u64, [0; 3]) }
pub fn sys_setpgid(pid: i32, pgid: i32) -> i64 {
    syscall(Syscall::Setpgid as u64, [pid as u64, pgid as u64, 0]) as i64
}
pub fn sys_getpgrp() -> i64 { syscall(Syscall::Getpgrp as u64, [0; 3]) as i64 }
pub fn sys_setsid() -> i64 { syscall(Syscall::Setsid as u64, [0; 3]) as i64 }
pub fn sys_ioctl(fd: u64, command: u32, argument: u64) -> i64 {
    syscall(Syscall::Ioctl as u64, [fd, command as u64, argument]) as i64
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
pub fn sys_getpid() -> u64 {
    syscall(Syscall::GetPid as _, [0, 0, 0])
}
pub fn sys_getppid() -> u64 {
    syscall(Syscall::GetPpid as _, [0, 0, 0])
}
pub fn sys_exit_group(code: u64) -> ! {
    syscall(Syscall::ExitGroup as _, [code, 0, 0]);
    unreachable!()
}
pub fn sys_set_tid_address(ptr: *mut u32) -> u64 {
    syscall(Syscall::SetTidAddress as _, [ptr as u64, 0, 0])
}
pub fn sys_arch_prctl(code: u64, addr: u64) -> u64 {
    syscall(Syscall::ArchPrctl as _, [code, addr, 0])
}
pub fn sys_uname(ptr: *mut u8) -> u64 {
    syscall(Syscall::Uname as _, [ptr as u64, 0, 0])
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
    use mos_abi::time::Timespec;
    let ts = Timespec {
        tv_sec: (ms / 1000) as i64,
        tv_nsec: ((ms % 1000) * 1_000_000) as i64,
    };
    syscall(
        Syscall::Nanosleep as _,
        [&ts as *const Timespec as u64, 0, 0],
    )
}
pub fn sys_fork() -> u64 {
    syscall(Syscall::Fork as _, [0, 0, 0])
}
pub fn sys_brk(addr: u64) -> u64 {
    syscall(Syscall::Brk as _, [addr, 0, 0])
}
pub fn sys_wait(pid: u64) -> u64 {
    let mut status = 0i32;
    let result = sys_wait4(pid as i32, &mut status, 0);
    if result < 0 { result as u64 }
    else if mos_abi::process::exited(status) { mos_abi::process::exit_code(status) as u64 }
    else { 128 + (status as u64 & 0x7f) }
}

pub fn sys_wait4(pid: i32, status: &mut i32, options: u32) -> i64 {
    syscall_v2(Syscall::Wait as _, [pid as u64, status as *mut i32 as u64, options as u64, 0, 0, 0]) as i64
}
pub fn sys_getdents(fd: u64, buf: &mut [u8]) -> u64 {
    syscall(
        Syscall::GetDents64 as _,
        [fd, buf.as_mut_ptr() as _, buf.len() as u64],
    )
}
pub fn sys_lseek(fd: u64, offset: i64, whence: u32) -> isize {
    syscall(Syscall::Lseek as _, [fd, offset as u64, whence as u64]) as _
}
pub fn sys_fstat(fd: u64, stat: *mut Stat) -> isize {
    syscall(Syscall::Fstat as _, [fd, stat as u64, 0]) as _
}
pub fn sys_stat(path: *const c_char, stat: *mut Stat) -> isize {
    syscall(Syscall::Stat as _, [path as u64, stat as u64, 0]) as _
}
pub fn sys_clock_gettime(clk_id: u32, tp: *mut Timespec) -> isize {
    syscall(Syscall::ClockGetTime as _, [clk_id as u64, tp as u64, 0]) as _
}
pub fn sys_gettimeofday(tv: *mut Timeval, tz: *mut Timezone) -> isize {
    syscall(Syscall::GetTimeOfDay as _, [tv as u64, tz as u64, 0]) as _
}
pub fn sys_chdir(fd: u64) -> u64 {
    syscall(Syscall::Fchdir as _, [fd, 0, 0])
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
pub fn sys_mprotect(addr: u64, len: u64, prot: u32) -> isize {
    syscall(Syscall::Mprotect as _, [addr, len, prot as u64]) as _
}
pub fn sys_madvise(addr: u64, len: u64, advice: u32) -> isize {
    syscall(Syscall::Madvise as _, [addr, len, advice as u64]) as _
}
pub fn sys_mkdir(path: *const c_char, mode: u32) -> isize {
    syscall(Syscall::Mkdir as _, [path as u64, mode as u64, 0]) as _
}
pub fn sys_unlink(path: *const c_char) -> isize {
    syscall(Syscall::Unlink as _, [path as u64, 0, 0]) as _
}
pub fn sys_rmdir(path: *const c_char) -> isize {
    syscall(Syscall::Rmdir as _, [path as u64, 0, 0]) as _
}
pub fn sys_rename(old: *const c_char, new: *const c_char) -> isize {
    syscall(Syscall::Rename as _, [old as u64, new as u64, 0]) as _
}
pub fn sys_link(old: *const c_char, new: *const c_char) -> isize {
    syscall(Syscall::Link as _, [old as u64, new as u64, 0]) as _
}
pub fn sys_symlink(target: *const c_char, linkpath: *const c_char) -> isize {
    syscall(Syscall::Symlink as _, [target as u64, linkpath as u64, 0]) as _
}
pub fn sys_readlink(path: *const c_char, buf: &mut [u8]) -> isize {
    syscall(
        Syscall::Readlink as _,
        [path as u64, buf.as_mut_ptr() as u64, buf.len() as u64],
    ) as _
}
pub fn sys_access(path: *const c_char, mode: u32) -> isize {
    syscall(Syscall::Access as _, [path as u64, mode as u64, 0]) as _
}
pub fn sys_umask(mask: u32) -> u32 {
    syscall(Syscall::Umask as _, [mask as u64, 0, 0]) as _
}
pub fn sys_truncate(path: *const c_char, len: u64) -> isize {
    syscall(Syscall::Truncate as _, [path as u64, len, 0]) as _
}
pub fn sys_ftruncate(fd: u64, len: u64) -> isize {
    syscall(Syscall::Ftruncate as _, [fd, len, 0]) as _
}
pub fn sys_chmod(path: *const c_char, mode: u32) -> isize {
    syscall(Syscall::Chmod as _, [path as u64, mode as u64, 0]) as _
}
pub fn sys_fchmod(fd: u64, mode: u32) -> isize {
    syscall(Syscall::Fchmod as _, [fd, mode as u64, 0]) as _
}
pub fn sys_fcntl(fd: u64, cmd: u32, arg: u64) -> isize {
    syscall(Syscall::Fcntl as _, [fd, cmd as u64, arg]) as _
}
pub fn sys_dup3(src: u64, dst: u64, flags: u32) -> isize {
    syscall(Syscall::Dup3 as _, [src, dst, flags as u64]) as _
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
