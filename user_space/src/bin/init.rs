#![no_std]
#![no_main]

use user_space::{exec, fork, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();
    let pid = fork();
    if pid == 0 {
        exec("shell", 2);
        unreachable!()
    } else {
        println!("[init]");
    }
    0
}
