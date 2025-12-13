#![no_std]
#![no_main]

use user_space::println;
#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_exit -----");
    0
}
