#![no_std]
#![no_main]

use user_space::{close, fork, init_heap, open, println, read, waitpid, write};
use vfs::OpenFlags;

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();
    println!("test_file start");
    let elf = open("./test_write", OpenFlags::READ);
    println!("test_file open success");
    let mut buffer = [0; 20];
    let n = read(elf, &mut buffer);
    println!("read: {}, data: {:?}", n, buffer);

    println!("----------------------");

    let txt = open(
        "./log.txt",
        OpenFlags::READ_WRITE | OpenFlags::CREATE | OpenFlags::APPEND,
    );
    write(txt, b"hello world");
    let child = fork();
    if child == 0 {
        println!("[child] write txt");
        write(txt, b", from child!");
        close(txt);
    } else {
        println!("[parent] waitpid");
        waitpid(child);
        println!("[parent] wake");
        let f = open("./log.txt", OpenFlags::READ);
        let mut buffer = [0; 128];
        println!("[parent] read file f");
        let n = read(f, &mut buffer);
        println!("read n: {}", n);
        println!("s: {:?}", core::str::from_utf8(&buffer[..n as usize]));
        close(f);
        close(txt);
    }
    0
}
