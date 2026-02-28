#![no_std]
#![no_main]

use user_space::{STDOUT, write, sleep};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let msg = "Hello, World! From test_write, --hlt loop\n";
    write(STDOUT, msg.as_bytes());
    sleep(100);
    0
}
