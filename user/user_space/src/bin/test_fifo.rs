#![no_std]
#![no_main]

use user_lib::fs::VfsError;
use user_lib::mos::fs::{O_RDONLY, O_WRONLY};
use user_lib::{exit, fork, mkfifo, open, println, read, write};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let ret = mkfifo(c"/fifo");
    match ret {
        Ok(_) => {
            println!("mkfifo ok");
        }
        Err(VfsError::AlreadyExists) => {
            println!("mkfifo already exists");
        }
        Err(e) => {
            println!("mkfifo error: {:?}", e);
            return -1;
        }
    }

    let pid = fork();
    if (pid as isize) < 0 {
        println!("fork failed");
        return -1;
    }
    if pid == 0 {
        println!("ready to open read fifo");
        let fifo = open(c"/fifo", O_RDONLY).expect("should open read fifo");
        let mut buffer = [0; 32];
        let n = read(fifo, &mut buffer).expect("read from pipe");
        println!(
            "read message: n: {}, s: {:?}",
            n,
            core::str::from_utf8(&buffer[..n])
        );
        exit(0);
    } else {
        println!("ready to open write fifo");
        let fifo = open(c"/fifo", O_WRONLY).expect("should open write fifo");
        println!("open fifo ok, ready to write fifo");
        write(fifo, b"message from fifo writer\n").expect("write to pipe");
    }

    0
}
