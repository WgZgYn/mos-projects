#![no_std]
#![no_main]

use core::ffi::c_char;

#[link(name = "test_dyn", kind = "dylib")]
unsafe extern "C" {
    fn c_plugin_add(a: i32, b: i32) -> i32;
    fn hello() -> *const c_char;
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> i32 {
    unsafe { c_plugin_add(4, 6 + 7) }
}

#[panic_handler]
fn panic_handler(_panic_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
