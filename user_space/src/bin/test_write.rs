#![no_std]
#![no_main]

use user_space::{STDOUT, write};

#[unsafe(no_mangle)]
fn main() -> u64 {
    let msg = "Hello, World! From test_write, --hlt loop\n";
    write(STDOUT, msg.as_bytes());
    0
}
