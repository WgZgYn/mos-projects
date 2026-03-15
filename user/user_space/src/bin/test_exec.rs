#![no_std]
#![no_main]

use user_lib::{exec, exit, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("--- test_exec ---");
    if let Err(e) = exec(c"/test_write") {
        println!("exec failed");
        exit(e as _);
    }
    unreachable!("bug: return from exec")
}
