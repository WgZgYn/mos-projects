#![no_std]
#![no_main]
#![feature(linkage)]

use crate::syscall::{
    sys_brk, sys_close, sys_exec, sys_exit, sys_fork, sys_getdents, sys_open, sys_rand, sys_read,
    sys_sleep, sys_wait, sys_write, sys_yield,
};
use core::arch::global_asm;
use linked_list_allocator::LockedHeap;
use vfs::{OpenFlags, UserDirEntry};

pub mod console;
mod syscall;

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

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
    loop {}
}

pub const STDOUT: u64 = 1;
pub const STDIN: u64 = 0;

pub fn write(fd: u64, buffer: &[u8]) -> u64 {
    sys_write(fd, buffer)
}
pub fn read(fd: u64, buffer: &mut [u8]) -> u64 {
    sys_read(fd, buffer)
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

pub fn exit(code: i32) -> u64 {
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
pub fn exec(path: &str, priority: u8) -> u64 {
    sys_exec(path, priority)
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
pub fn open(path: &str, flags: OpenFlags) -> u64 {
    sys_open(path, flags.bits())
}
pub fn close(fd: u64) -> u64 {
    sys_close(fd)
}
pub fn getdents(fd: u64, buf: &mut [UserDirEntry]) -> u64 {
    sys_getdents(fd, buf)
}
