#![no_std]
#![no_main]
#![feature(linkage)]

use crate::syscall::{
    sys_brk, sys_chdir, sys_close, sys_execve, sys_exit, sys_fork, sys_getdents, sys_mmap,
    sys_munmap, sys_open, sys_rand, sys_read, sys_sleep, sys_wait, sys_write, sys_yield,
};
use core::arch::global_asm;
use core::ffi::{CStr, c_char};
use linked_list_allocator::LockedHeap;
use mos_lib::fs::FileDesc;
use mos_lib::memory::{MemoryMapFlags, MemoryProtFlags};
use vfs::{OpenFlags, UserDirEntry};

pub mod console;
mod syscall;

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

pub type Args = *const *const c_char;

pub fn init_heap() {
    let old_brk = brk(0);
    unsafe {
        if brk(old_brk + 4096 * 32) == u64::MAX {
            // 128 KB
            panic!("failed to set heap brk");
        }
        let new_brk = brk(0);

        let size = new_brk as usize - old_brk as usize;
        HEAP.lock().init(old_brk as _, size);

        // let guard = HEAP.lock();
        // println!(
        //     "heap_info, size: {}, top: {:?}, bottom: {:?}, free: {}, used: {}",
        //     guard.size(),
        //     guard.top(),
        //     guard.bottom(),
        //     guard.free(),
        //     guard.used()
        // );
        // println!("new_brk: {:x?}, old_brk: {:x?}", new_brk, old_brk);
    }
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> i32 {
    panic!("no main")
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
    mov rdi, rax
    mov rax, {exit_syscall_no}
    syscall
    ",
    main = sym main,
    exit_syscall_no = const 60
);

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
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
    exit(-1)
}

pub const STDOUT: FileDesc = FileDesc(1);
pub const STDIN: FileDesc = FileDesc(0);

pub fn write(fd: FileDesc, buffer: &[u8]) -> u64 {
    sys_write(fd.0 as _, buffer)
}
pub fn read(fd: FileDesc, buffer: &mut [u8]) -> u64 {
    sys_read(fd.0 as _, buffer)
}

pub fn get_char() -> u8 {
    let mut c = b'\0';
    read(STDIN, core::slice::from_mut(&mut c));
    c
}

pub fn write_stdout_number(mut v: usize) -> u64 {
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
pub fn open(path: &str, flags: OpenFlags) -> Option<FileDesc> {
    let fd = sys_open(path, flags.bits());
    if (fd as isize) > 0 {
        Some(FileDesc(fd as _))
    } else {
        None
    }
}
pub fn close(fd: FileDesc) -> u64 {
    sys_close(fd.0 as _)
}
pub fn getdents(fd: FileDesc, buf: &mut [UserDirEntry]) -> u64 {
    sys_getdents(fd.0 as _, buf)
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
