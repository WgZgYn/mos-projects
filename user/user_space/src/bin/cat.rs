#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec;
use user_lib::fs::OpenFlags;
use user_lib::{Args, mos::ffi::NullEndPointerArray, open, print, println, read};

#[unsafe(no_mangle)]
fn main(argc: usize, argv: Args) -> i32 {
    let args = unsafe { NullEndPointerArray::from_ptr(argv) };
    let args = args.to_vec_cstring();
    if args.len() < 3 {
        println!("{} {:?}", argc, args);
        println!("too few arguments");
        return -1;
    }
    let program_name = &args[0];
    let filepath = &args[1];
    let length = &args[2];
    let n = length
        .as_ref()
        .to_str()
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let Some(n) = n else {
        println!("`{:?}` is not a legal number", length);
        return -1;
    };

    let Some(fd) = open(filepath, OpenFlags::READ) else {
        println!("open {:?} failed", filepath);
        return -1;
    };

    let mut buffer = vec![0; n];
    let n = read(fd, &mut buffer);
    if n != u64::MAX {
        let chunks = buffer[..n as usize].chunks(20);
        for chunk in chunks {
            for c in chunk {
                print!("{:02x} ", c);
            }
            println!();
        }
        println!("read {} bytes from {:?}", n, filepath);
    } else {
        println!("read error");
        return -1;
    }
    0
}
