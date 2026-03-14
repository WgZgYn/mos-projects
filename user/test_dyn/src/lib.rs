#![no_std]
#![no_main]

use core::ffi::c_char;

#[unsafe(no_mangle)]
pub extern "C" fn c_plugin_add(a: i32, b: i32) -> i32 {
    a + b
}

#[unsafe(no_mangle)]
pub extern "C" fn hello() -> *const c_char {
    c"Hello, world!".as_ptr()
}

#[panic_handler]
fn panic_handler(_panic_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
