#![no_std]
#![no_main]
extern crate alloc;

use alloc::boxed::Box;
use alloc::string::String;
use bs_string::BString;
use user_space::{println, rand, write, yield_, STDOUT};


#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_rand ----- start");
    let mut r = rand();

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
    println!("----- test_rand ----- end");
    0
}
