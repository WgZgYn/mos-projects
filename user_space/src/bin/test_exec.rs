#![no_std]
#![no_main]

use user_space::{exec, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("--- test_exec ---");
    exec("/test_write", 3);
    unreachable!("bug: return from exec")
}
