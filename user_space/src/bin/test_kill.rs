#![no_std]
#![no_main]

use core::arch::asm;
use user_space::println;
#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_kill -----");
    unsafe {
        asm!("hlt");
    }
    0
}
