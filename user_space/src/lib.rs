#![no_std]
#![no_main]
#![feature(linkage)]

use crate::syscall::{sys_exit, sys_rand, sys_write, sys_yield};
use linked_list_allocator::LockedHeap;

pub mod console;
mod syscall;

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> u64 {
    panic!("no main")
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    init_heap();
    exit(main());
}

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

fn init_heap() {
    static mut DATA: [u8; 4096] = [0; 4096];
    unsafe { HEAP.lock().init(&raw mut DATA as _, 4096) };
}

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

pub fn write(fd: u64, buffer: &[u8]) -> u64 {
    sys_write(fd, buffer)
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

pub fn exit(code: u64) -> u64 {
    sys_exit(code)
}

pub fn rand() -> u64 {
    sys_rand()
}

pub fn yield_() -> u64 {
    sys_yield()
}
