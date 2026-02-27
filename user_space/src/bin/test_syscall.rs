#![no_std]
#![no_main]

use core::arch::asm;
use user_space::{STDOUT, println, write};

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("[test_syscall main]:");
    let mut ret: u64;
    unsafe {
        asm!("syscall", in("rax") 7, lateout("rax") ret);
    }
    let mut buffer = [0; 33];
    buffer[32] = b'\n';
    let mut len = 0;
    while ret > 0 {
        let index = buffer.len() - 2 - len;
        buffer[index] = (ret % 10) as u8 + b'0';
        ret /= 10;
        len += 1;
    }
    let offset = buffer.len() - len;
    write(STDOUT, &buffer[offset..]);
    0
}
