#![no_std]
#![no_main]

use user_lib::fs::{OpenFlags, VfsError};
use user_lib::{open, println, read, sleep};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("test nonblock");
    let nonblock = open(c"/dev/tty", OpenFlags::READ | OpenFlags::NONBLOCK).expect("can't open");
    let mut buffer = [0; 32];
    let n = read(nonblock, &mut buffer);
    println!("result: {:?}", n);
    loop {
        let ret = read(nonblock, &mut buffer);
        match ret {
            Ok(n) => {
                println!("result: {}, {:?}", n, &buffer[..n]);
                break;
            }
            Err(VfsError::Pendding) => {
                sleep(2);
            }
            Err(e) => {
                println!("error: {:?}", e);
                break;
            }
        }
    }
    0
}
