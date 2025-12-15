#![no_std]
#![no_main]

use core::arch::asm;
use user_space::println;
#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_kill ----- start");
    unsafe {
        asm!("hlt");
    }
    println!("----- test_kill ----- end");
    0
}
