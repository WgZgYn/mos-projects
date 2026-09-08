#![no_std]
#![no_main]

use core::str;
use user_lib::mos::fs::{DT_BLK, DT_CHR, DT_DIR, DT_FIFO, DT_REG, O_DIRECTORY, O_RDONLY};
use user_lib::{getdents, open, println};

fn type_str(t: u8) -> &'static str {
    match t {
        DT_DIR => "dir",
        DT_REG => "file",
        DT_FIFO => "fifo",
        DT_CHR => "chr",
        DT_BLK => "blk",
        _ => "?",
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let current_dir = open(c"./", O_RDONLY | O_DIRECTORY).expect("should open directory");
    let mut buf = [0u8; 4096];
    let n = getdents(current_dir, &mut buf) as isize;
    if n < 0 {
        println!("ls: getdents failed: {}", n);
        return 1;
    }
    let mut pos = 0usize;
    while pos < n as usize {
        let rec = &buf[pos..];
        let reclen = u16::from_ne_bytes([rec[16], rec[17]]) as usize;
        if reclen == 0 || pos + reclen > n as usize {
            break;
        }
        let ino = u64::from_ne_bytes(rec[0..8].try_into().unwrap());
        let name = rec[19..reclen].split(|&b| b == 0).next().unwrap_or(&[]);
        let name = str::from_utf8(name).unwrap_or("?");
        println!("{}\t{}\t{}", ino, type_str(rec[18]), name);
        pos += reclen;
    }
    0
}
