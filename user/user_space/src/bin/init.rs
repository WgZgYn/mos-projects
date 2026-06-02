#![no_std]
#![no_main]

use user_lib::{execve, fork, println, waitpid};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[init] running main");
    let pid = fork();
    if pid == 0 {
        let args = &[
            c"/shell".as_ptr(),
            c"hello".as_ptr(),
            c"world".as_ptr(),
            core::ptr::null(),
        ];
        let envp = &[c"PATH=/".as_ptr(), core::ptr::null()];
        println!("[init] create child process shell");
        execve(c"shell", args, envp).expect("should execute shell");
        unreachable!()
    } else {
        let ret = waitpid(pid);
        if ret == 0 {
            println!("child Pid({}) exited successfully", pid);
        } else {
            println!("child Pid({}) exited with error {}", pid, ret);
        }
    }
    0
}
