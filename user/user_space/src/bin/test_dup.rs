#![no_std]
#![no_main]

use user_lib::fs::OpenFlags;
use user_lib::{STDOUT, dup, dup2, exec, exit, fork, open, println, write};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[test_dup main]:");
    let dup_out = dup(STDOUT).expect("dup failed");
    write(dup_out, b"write to dup fd\n").expect("write failed");
    let f = open(c"/dup.out", OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNC).expect("open failed");
    if !dup2(f, STDOUT) {
        println!("[test_dup] dup2 failed");
        exit(1);
    }
    println!("[test_dup] write to dup file");

    let ret = fork();
    if (ret as isize) < 0 {
        println!("[test_dup] fork failed");
        exit(1);
    }
    if ret == 0 {
        write(dup_out, b"child process\n").expect("write failed");
        if let Err(e) = exec(c"/test_write") {
            exit(e as _);
        } else {
            exit(0);
        }
    } else {
        exit(0)
    }
    0
}
