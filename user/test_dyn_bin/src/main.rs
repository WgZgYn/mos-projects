#![no_std]
#![no_main]

use core::ffi::c_char;
use user_lib::{write, STDOUT};

#[link(name = "test_dyn", kind = "dylib")]
unsafe extern "C" {
    fn c_plugin_add(a: i32, b: i32) -> i32;
    fn hello() -> *const c_char;
}

#[unsafe(no_mangle)]
pub fn main() -> i32 {
    let ret = unsafe { c_plugin_add(4, 6 + 7) };
    if ret == 17 {
        let message = b"You did it!!!\n";
        write(STDOUT, message);
        unsafe {
            let hello = hello();
            let mut i = 0;
            while *hello.add(i) != 0 {
                i += 1;
            }
            write(STDOUT, core::slice::from_raw_parts(hello as _, i));
        }
    }
    0
}
