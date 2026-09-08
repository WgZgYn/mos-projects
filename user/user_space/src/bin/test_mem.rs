#![no_std]
#![no_main]

use user_lib::mos::fs::{O_CREAT, O_RDONLY, O_RDWR};
use user_lib::{close, exit, open, println, read, write};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("test memfs");

    let f = open(c"/mem/temp.txt", O_CREAT | O_RDWR)
        .expect("failed to open /mem/temp.txt");

    let Ok(_) = write(f, b"Hello World!\n") else {
        println!("failed to write to /mem/temp.txt");
        exit(-1);
    };
    close(f);

    let f = open(c"/mem/temp.txt", O_RDONLY).expect("failed to open /mem/temp.txt");
    let mut buffer = [0; 20];
    let Ok(n) = read(f, &mut buffer) else {
        println!("failed to read from /mem/temp.txt");
        exit(-1);
    };
    println!("read {} bytes", n);
    println!("s: `{}`", core::str::from_utf8(&buffer[..n]).unwrap());
    0
}
