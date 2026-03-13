#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn c_plugin_add(a: i32, b: i32) -> i32 {
    a + b
}

pub fn rust_plugin_add(a: i32, b: i32) -> i32 {
    a + b
}

#[unsafe(no_mangle)]
pub fn hello() -> &'static str {
    "Hello, world!"
}

#[panic_handler]
fn panic_handler(_panic_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
