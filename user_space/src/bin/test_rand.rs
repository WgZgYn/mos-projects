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
use user_space::{println, rand, write, write_stdout_number, yield_, STDOUT};



pub fn init_heap() {
    static mut DATA: [u8; 4096] = [0; 4096];
    unsafe { HEAP.lock().init(&raw mut DATA as _, 4096) };
}

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_rand ----- start");
    let mut r = rand();
    init_heap();

    let mut bs = unsafe { Box::<[u8; 10]>::new_uninit().assume_init() };
    for i in 0..10 {
        bs[i] = b'a' + i as u8;
    }
    write(STDOUT, bs.as_slice());
    let mut s = String::with_capacity(10);
    for i in 0..10 {
        s.push('a');
    }
    write(STDOUT, s.as_bytes());

    let mut s = BString::<32>::new();
    println!("BString: ");
    while r > 0 {
        s.push_byte((r % 10) as u8 + b'0');
        r /= 10;
    }
    s.reverse();

    println!("--- call yield ---");
    yield_();
    write(STDOUT, s.as_bytes());

    for i in 0..10 {
        write_stdout_number(i);
    }

    let v = 5;
    let mut buffer = String::with_capacity(32);
    buffer.push_str("v: ");
    buffer.push_str(v.to_string().as_str());
    write(STDOUT, buffer.as_bytes());

    let mut nums = Vec::new();
    for i in 0..10 {
        nums.push(i * 2);
    }
    let v = 5;
    println!("test println! const: {}, var: {}, arr: {:?}, success", 4, v, nums);

    println!("----- test_rand ----- end");
    0
}
