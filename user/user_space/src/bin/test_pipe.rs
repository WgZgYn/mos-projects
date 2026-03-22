#![no_std]
#![no_main]

use user_lib::fs::VfsError;
use user_lib::{close, fork, get_char, pipe, println, read, waitpid, write, Pipe, STDOUT};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let pipe = pipe();
    let Some(Pipe { pread, pwrite }) = pipe else {
        println!("make pipe failed");
        return -1;
    };

    let ret = fork();
    if ret == u64::MAX {
        println!("fork failed");
        return -1;
    }

    if ret == 0 {
        close(pwrite);
        let mut buf = [b'\0'];
        loop {
            let ret = read(pread, &mut buf);
            match ret {
                Ok(0) => {
                    println!("read finished successfully");
                    break;
                }
                Ok(_) => {
                    write(STDOUT, &buf).expect("should write");
                }
                Err(VfsError::BrokenPipe) => {
                    println!("pipe broken");
                    break;
                }
                Err(e) => panic!("read error: {:?}", e),
            }
        }
        write(STDOUT, &[b'\n']).expect("should write");
        0
    } else {
        close(pread);
        loop {
            let c = get_char();
            if c == b'Q' {
                break;
            }
            write(pwrite, core::slice::from_ref(&c)).expect("should write");
        }
        write(STDOUT, b"parent writing finished\n").expect("should write");
        close(pwrite);
        waitpid(ret);
        0
    }
}
