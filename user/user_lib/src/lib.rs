#![no_std]
#![no_main]
#![feature(linkage)]

pub mod console;
mod os;
mod syscall;

use crate::syscall::{
    sys_brk, sys_chdir, sys_clock_freq, sys_clock_gettime, sys_clock_ticks, sys_close,
    sys_create_thread, sys_create_window, sys_destroy_window, sys_dup, sys_dup2, sys_execve,
    sys_exit, sys_fork, sys_fstat, sys_getcpu, sys_getdents, sys_gettimeofday, sys_lseek,
    sys_madvise, sys_mkfifo, sys_mmap, sys_mprotect, sys_munmap, sys_open, sys_pipe, sys_rand,
    sys_read, sys_sem_create, sys_sem_signal, sys_sem_wait, sys_sleep, sys_stat, sys_wait,
    sys_window_mark_dirty, sys_window_sync, sys_write, sys_yield,
};
use core::arch::global_asm;
use core::ffi::{CStr, c_char};
use mos_abi::fs::{FileDesc, Stat};
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_abi::time::{Timespec, Timeval, Timezone};
use vfs_common::{VfsResult, isize_to_vfs_result};

#[cfg(feature = "alloc")]
use linked_list_allocator::LockedHeap;

pub use mos_abi as mos;
pub use vfs_common as fs;

/// Sometime if the user_lib is a pure lib,
/// it needs to include this symbol to make sure use the same user panic handler
pub const __LINK_MARK: () = ();

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main(argc: usize, argv: Args, envp: Args) -> i32 {
    panic!("no main")
}

extern "C" fn runtime(argc: usize, argv: Args, envp: Args) -> ! {
    #[cfg(feature = "alloc")]
    init_heap();
    exit(main(argc, argv, envp));
}

global_asm!(
    "
.global _start
_start:
    // argc
    mov rdi, [rsp + 0x00]

    // argv
    lea rsi, [rsp + 0x08]

    // envp
    mov rdx, rdi
    shl rdx, 3
    add rdx, rsi
    add rdx, 8

    // abi & align down
    xor rbp, rbp
    and rsp, -16
    call {main}
    ",
    main = sym runtime,
);

#[panic_handler]
pub fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    #[cfg(feature = "alloc")]
    {
        let err = panic_info.message();
        if let Some(location) = panic_info.location() {
            println!(
                "Panicked at {}:{}, {}",
                location.file(),
                location.line(),
                err
            );
        } else {
            println!("Panicked: {}", err);
        }
    }
    exit(-1)
}

pub type Args = *const *const c_char;

pub const STDOUT: FileDesc = FileDesc(1);
pub const STDIN: FileDesc = FileDesc(0);

#[cfg(feature = "alloc")]
#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

#[cfg(feature = "alloc")]
pub fn init_heap() {
    let old_brk = brk(0);
    unsafe {
        if is_err(brk(old_brk + 4096 * 32)) {
            // 128 KB
            panic!("failed to set heap brk");
        }
        let new_brk = brk(0);

        let size = new_brk as usize - old_brk as usize;
        HEAP.lock().init(old_brk as _, size);
    }
}

pub fn write(fd: FileDesc, buffer: &[u8]) -> VfsResult<usize> {
    isize_to_vfs_result(sys_write(fd.0 as _, buffer))
}
pub fn read(fd: FileDesc, buffer: &mut [u8]) -> VfsResult<usize> {
    isize_to_vfs_result(sys_read(fd.0 as _, buffer))
}

/// Linux errno convention: a negative rax means -errno.
pub fn is_err(ret: u64) -> bool {
    (ret as i64) < 0
}

pub fn get_char() -> u8 {
    let mut c = b'\0';
    read(STDIN, core::slice::from_mut(&mut c));
    c
}

pub fn write_stdout_number(mut v: usize) -> VfsResult<usize> {
    let mut buffer = [b'\0'; 32];
    if v == 0 {
        return write(STDOUT, b"0");
    }
    let mut i = 0;
    while v > 0 {
        buffer[i] = (v % 10) as u8 + b'0';
        i += 1;
        v /= 10;
    }
    buffer[..i].reverse();
    write(STDOUT, &buffer[..i])
}

pub fn exit(code: i32) -> ! {
    sys_exit(code as _)
}

pub fn rand() -> u64 {
    sys_rand()
}

pub fn yield_() {
    sys_yield();
}
pub fn getcpu() -> u64 {
    sys_getcpu()
}
pub fn sleep(ms: u64) {
    sys_sleep(ms);
}
pub fn exec(path: &CStr) -> Result<(), isize> {
    execve(path, &[core::ptr::null()], &[core::ptr::null()])
}
pub fn execve(path: &CStr, args: &[*const c_char], envp: &[*const c_char]) -> Result<(), isize> {
    if let Some(ptr) = args.last()
        && ptr.is_null()
    {
        let ret = sys_execve(path.as_ptr(), args.as_ptr(), envp.as_ptr());
        if ret != 0 { Err(ret) } else { Ok(()) }
    } else {
        Err(-1)
    }
}
pub fn fork() -> u64 {
    sys_fork()
}
pub fn brk(addr: u64) -> u64 {
    sys_brk(addr)
}
pub fn waitpid(pid: u64) -> i32 {
    sys_wait(pid) as _
}
/// open(2) with Linux O_* flags (see mos_abi::fs::O_*).
pub fn open(path: &CStr, flags: u32) -> Option<FileDesc> {
    let fd = sys_open(path.as_ptr(), flags);
    if (fd as i64) >= 0 {
        Some(FileDesc(fd as _))
    } else {
        None
    }
}
pub fn close(fd: FileDesc) -> u64 {
    sys_close(fd.0 as _)
}
/// getdents64(2): buf is filled with Linux dirent64 records (mos_abi::fs::Dirent64).
pub fn getdents(fd: FileDesc, buf: &mut [u8]) -> u64 {
    sys_getdents(fd.0 as _, buf)
}
pub fn lseek(fd: FileDesc, offset: i64, whence: u32) -> isize {
    sys_lseek(fd.0 as _, offset, whence)
}
pub fn fstat(fd: FileDesc, stat: &mut Stat) -> isize {
    sys_fstat(fd.0 as _, stat as *mut Stat)
}
pub fn stat(path: &CStr, stat: &mut Stat) -> isize {
    sys_stat(path.as_ptr(), stat as *mut Stat)
}
/// clock_gettime(2) convenience: returns the Timespec, or the kernel's
/// errno as a negative value in tv_sec.
pub fn clock_gettime(clk_id: u32) -> Timespec {
    let mut ts = Timespec::default();
    let ret = sys_clock_gettime(clk_id, &mut ts as *mut Timespec);
    if ret != 0 {
        ts.tv_sec = ret as i64;
    }
    ts
}
pub fn gettimeofday(tv: &mut Timeval, tz: &mut Timezone) -> isize {
    sys_gettimeofday(tv as *mut Timeval, tz as *mut Timezone)
}
pub fn chdir(fd: FileDesc) -> u64 {
    sys_chdir(fd.0 as _)
}
pub fn mmap(
    addr: u64,
    len: u64,
    prot: MemoryProtFlags,
    flags: MemoryMapFlags,
    fd: Option<FileDesc>,
    offset: u64,
) -> u64 {
    sys_mmap(
        addr,
        len,
        prot.bits(),
        flags.bits(),
        fd.map(|f| f.0 as _).unwrap_or(-1),
        offset,
    )
}

pub fn munmap(addr: u64, len: u64) -> u64 {
    sys_munmap(addr, len)
}

pub fn mprotect(addr: u64, len: u64, prot: u32) -> isize {
    sys_mprotect(addr, len, prot)
}

pub fn madvise(addr: u64, len: u64, advice: u32) -> isize {
    sys_madvise(addr, len, advice)
}

pub struct Pipe {
    pub pread: FileDesc,
    pub pwrite: FileDesc,
}

pub fn pipe() -> Option<Pipe> {
    let mut fd = [-1, -1];
    if sys_pipe(&mut fd) == 0 {
        return Some(Pipe {
            pread: FileDesc(fd[0] as _),
            pwrite: FileDesc(fd[1] as _),
        });
    }
    None
}

pub fn mkfifo(path: &CStr) -> VfsResult<()> {
    isize_to_vfs_result(sys_mkfifo(path)).map(|_| ())
}
pub fn dup(fd: FileDesc) -> Option<FileDesc> {
    let ret = sys_dup(fd.0 as _);
    if ret >= 0 {
        Some(FileDesc(ret as _))
    } else {
        None
    }
}

pub fn dup2(src: FileDesc, dst: FileDesc) -> bool {
    let ret = sys_dup2(src.0 as _, dst.0 as _);
    ret == 0
}

pub fn create_thread(entry: *const fn() -> !, stack: *mut u64) -> u64 {
    sys_create_thread(entry as _, stack as _)
}

pub fn sem_create(initial: u64) -> u64 {
    sys_sem_create(initial)
}
pub fn sem_signal(sem_id: u64) {
    sys_sem_signal(sem_id)
}
pub fn sem_wait(sem_id: u64) {
    sys_sem_wait(sem_id)
}

/// PM-timer 单调时钟 (ticks, 64-bit 累积值)
pub fn clock_ticks() -> u64 {
    sys_clock_ticks()
}
/// PM-timer 频率 (Hz, 通常为 3579545)
pub fn clock_freq() -> u64 {
    sys_clock_freq()
}
/// 单调时间 (微秒)
pub fn clock_us() -> u64 {
    let ticks = sys_clock_ticks();
    (ticks as u128 * 1_000_000 / sys_clock_freq() as u128) as u64
}

// ─── 用户态窗口 API ─────────────────────────────

/// create_window 返回信息
#[repr(C)]
pub struct WindowInfo {
    pub id: u64,
    pub pixels_ptr: u64,
    pub width: u64,
    pub height: u64,
    pub pitch: u64,
}

/// 创建一个窗口，返回共享像素缓冲区的用户态指针。
/// 用户直接写入 RGBA 像素 (每像素 4 字节, 行优先, pitch 步长)。
pub fn create_window(x: u64, y: u64, w: u64, h: u64) -> Option<WindowInfo> {
    let mut out = [0u64; 5];
    let ret = sys_create_window(x, y, w, h, out.as_mut_ptr());
    if is_err(ret) {
        return None;
    }
    Some(WindowInfo {
        id: out[0],
        pixels_ptr: out[1],
        width: out[2],
        height: out[3],
        pitch: out[4],
    })
}

/// 标记窗口内的脏矩形 (窗口坐标系, 0,0 为左上角)。
/// 可多次调用累积多个脏区域。
pub fn window_mark_dirty(win_id: u64, x: u64, y: u64, w: u64, h: u64) {
    sys_window_mark_dirty(win_id, x, y, w, h)
}

/// 将所有窗口的累积脏区域提交到帧缓冲。
pub fn window_sync() {
    sys_window_sync()
}

/// 销毁窗口并释放其物理帧。
pub fn destroy_window(win_id: u64) {
    sys_destroy_window(win_id)
}
