#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use user_lib::fs::{OpenFlags, UserDirEntry};
use user_lib::{getdents, open, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let current_dir =
        open(c"./", OpenFlags::READ | OpenFlags::DIRECTORY).expect("should open directory");
    let mut buffer = [const { MaybeUninit::<UserDirEntry>::uninit() }; 32];
    unsafe {
        let ptr = buffer.as_mut_ptr().cast::<UserDirEntry>();
        let len = buffer.len();
        let counts = getdents(current_dir, core::slice::from_raw_parts_mut(ptr, len));
        let entries = buffer[..counts as usize].assume_init_ref();
        for entry in entries {
            if let Some(name) = entry.name() {
                println!("{:?}\t{}\t{}", entry.file_type, entry.size, name);
            }
        }
    }
    0
}
