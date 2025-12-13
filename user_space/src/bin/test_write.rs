#![no_std]
#![no_main]

use user_space::{write, STDOUT};

#[unsafe(no_mangle)]
fn main() -> u64 {
    let msg = "Hello, World! From test_syscall, --hlt loop\n";
    write(STDOUT, msg.as_bytes());
    0
}
