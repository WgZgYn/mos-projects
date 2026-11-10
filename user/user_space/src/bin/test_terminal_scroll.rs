#![no_std]
#![no_main]
extern crate alloc;
use alloc::string::String;
use core::fmt::Write;
use user_lib::mos::fs::{TIOCGWINSZ, Winsize};
use user_lib::{STDIN, STDOUT, ioctl, write};
#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut size = Winsize::default();
    assert_eq!(ioctl(STDIN, TIOCGWINSZ, &mut size as *mut _ as u64), 0);
    let mut text = String::from("\x1b[0m\x1b[2J\x1b[H");
    for i in 0..size.ws_row as usize + 12 {
        writeln!(text, "\x1b[32mSCROLL ROW {:03}\x1b[0m retained text", i).unwrap();
    }
    text.push_str("[terminal-scroll] PASS\n");
    assert_eq!(write(STDOUT, text.as_bytes()).unwrap(), text.len());
    0
}
