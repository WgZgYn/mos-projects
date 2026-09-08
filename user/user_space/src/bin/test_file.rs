#![no_std]
#![no_main]

use user_lib::mos::fs::{O_APPEND, O_CREAT, O_DIRECTORY, O_RDONLY, O_RDWR};
use user_lib::{chdir, close, fork, open, println, read, waitpid, write};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("test_file start");
    let elf = open(c"./test_write", O_RDONLY).expect("should open test_write");
    println!("test_file open success");
    let mut buffer = [0; 20];
    let n = read(elf, &mut buffer);
    println!("read: {:?}, data: {:?}", n, buffer);

    println!("----------------------");

    let txt = open(
        c"./log.txt",
        O_RDWR | O_CREAT | O_APPEND,
    )
    .expect("should open log.txt");
    write(txt, b"hello world").expect("should write file");
    let child = fork();
    if child == 0 {
        println!("[child] write txt");
        write(txt, b", from child!").expect("should write file");
        close(txt);
    } else {
        println!("[parent] waitpid");
        waitpid(child);
        println!("[parent] wake");
        let f = open(c"./log.txt", O_RDONLY).expect("should open log.txt");
        let mut buffer = [0; 128];
        println!("[parent] read file f");
        let n = read(f, &mut buffer).expect("read file");
        println!("read n: {}", n);
        println!("s: {:?}", core::str::from_utf8(&buffer[..n]));
        close(f);
        close(txt);
        test_dir();
    }
    0
}

fn test_dir() {
    let new_dir = open(
        c"./document",
        O_RDONLY | O_DIRECTORY | O_CREAT,
    )
    .expect("should open document");
    chdir(new_dir);
    let new_file = open(c"./data.bin", O_RDWR | O_CREAT)
        .expect("should open data.bin");
    write(new_file, b"hello world").expect("should write file");
    close(new_file);
    close(new_dir);
}
