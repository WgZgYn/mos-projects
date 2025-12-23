#![feature(allocator_api)]
#![no_std]
#![no_main]
extern crate alloc;

#[global_allocator]
pub static HEAP: LockedHeap = LockedHeap::empty();

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use bs_string::BString;
use linked_list_allocator::LockedHeap;
use user_space::{STDOUT, println, rand, write, write_stdout_number, yield_};

pub fn init_heap() {
    const HEAP_SIZE: usize = 10240;
    static mut DATA: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    unsafe { HEAP.lock().init(&raw mut DATA as _, HEAP_SIZE) };
}

fn test_box() {
    let mut bs = unsafe { Box::<[u8; 10]>::new_uninit().assume_init() };
    for i in 0..10 {
        bs[i] = b'a' + i as u8;
    }
    write(STDOUT, bs.as_slice());
    write(STDOUT, b"\n");
}

fn test_str() {
    let mut s = String::with_capacity(10);
    for _ in 0..10 {
        s.push('a');
    }
    write(STDOUT, s.as_bytes());
    write(STDOUT, b"\n");
}

fn test_bstr() {
    let mut r = rand();
    let mut s = BString::<32>::new();
    println!("BString: ");
    while r > 0 {
        s.push_byte((r % 10) as u8 + b'0');
        r /= 10;
    }
    s.reverse();
    write(STDOUT, s.as_bytes());
    write(STDOUT, b"\n");
}

fn test_yield() {
    println!("--- call yield ---");
    yield_();
}

fn test_write_number() {
    for i in 0..10 {
        write_stdout_number(i);
    }
}

fn test_string() {
    let v = 5;
    let mut buffer = String::with_capacity(32);
    buffer.push_str("v: ");
    buffer.push_str(v.to_string().as_str());
    buffer.push_str("\n");
    write(STDOUT, buffer.as_bytes());
}

fn test_fmt() {
    let mut nums = Vec::new();
    for i in 0..10 {
        nums.push(i * 2);
    }
    let v = 5;
    println!(
        "test println! const: {}, var: {}, arr: {:?}, success",
        4, v, nums
    );
}

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_rand ----- start");
    init_heap();

    test_box();
    test_str();
    test_yield();
    test_bstr();
    test_write_number();
    test_string();
    test_fmt();

    println!("----- test_rand ----- end");
    0
}
