#![no_std]
#![no_main]
use core::ffi::c_char;

const _: () = user_lib::__LINK_MARK;

#[unsafe(no_mangle)]
pub extern "C" fn c_plugin_add(a: i32, b: i32) -> i32 {
    a + b
}

#[unsafe(no_mangle)]
pub extern "C" fn hello() -> *const c_char {
    c"Hello, world!\n".as_ptr()
}
