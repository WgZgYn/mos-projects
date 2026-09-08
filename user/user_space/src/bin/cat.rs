#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec;
use user_lib::mos::fs::O_RDONLY;
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
    let str_fmt = if let Some(a) = args.get(3) {
        a == c"-s"
    } else {
        false
    };

    let n = length
        .as_ref()
        .to_str()
        .ok()
        .and_then(|s| s.parse::<usize>().ok());
    let Some(n) = n else {
        println!("`{:?}` is not a legal number", length);
        return -1;
    };

    let Some(fd) = open(filepath, O_RDONLY) else {
        println!("open {:?} failed", filepath);
        return -1;
    };

    let mut buffer = vec![0; n];
    let Ok(n) = read(fd, &mut buffer) else {
        println!("read {:?} failed", filepath);
        return -1;
    };

    if str_fmt {
        let Ok(s) = core::str::from_utf8(&buffer[..n]) else {
            println!("not valid utf-8");
            return -1;
        };
        println!("{}", s);
    } else {
        let chunks = buffer[..n].chunks(20);
        for chunk in chunks {
            for c in chunk {
                print!("{:02x} ", c);
            }
            println!();
        }
    }
    println!("read {} bytes from {:?}", n, filepath);
    0
}
