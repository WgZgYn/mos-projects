#![no_std]
#![no_main]
extern crate alloc;

use alloc::string::String;
use user_space::{get_char, init_heap, print, println};

#[unsafe(no_mangle)]
fn main() -> u64 {
    init_heap();
    println!("MOS Shell");
    println!(">> ");
    let mut line = String::new();
    loop {
        let c = get_char();
        match c {
            b'\n' => {
                print!("\nyou input {}\n>> ", line.len());
                line.clear();
            }
            _ => {
                line.push(c as char);
                print!("{}", c as char);
            }
        }
    }
}
