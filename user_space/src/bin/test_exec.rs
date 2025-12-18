#![no_std]
#![no_main]

use user_space::{exec, println};

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("--- test_exec ---");
    exec("/test_write", 2)
}
