#![no_std]
#![no_main]

use user_space::{execve, fork, init_heap, println, waitpid};

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();
    println!("[init]");
    let pid = fork();
    if pid == 0 {
        let args = &[
            c"/shell".as_ptr(),
            c"hello".as_ptr(),
            c"world".as_ptr(),
            core::ptr::null(),
        ];
        let envp = &[c"PATH=/".as_ptr(), core::ptr::null()];
        execve(c"shell", args, envp).expect("should execute shell");
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
