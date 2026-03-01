#![no_std]
#![no_main]

use user_space::{exec, fork, init_heap, println, waitpid};

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();
    let pid = fork();
    if pid == 0 {
        exec("shell", 2);
        unreachable!()
    } else {
        println!("[init] waitpid {}", pid);
        let ret = waitpid(pid);
        if ret == 0 {
            println!("child Pid({}) exited successfully", pid);
        } else {
            println!("child Pid({}) exited with error {}", pid, ret);
        }
    }
    0
}
