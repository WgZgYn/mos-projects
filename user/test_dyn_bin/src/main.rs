#![no_std]
#![no_main]

use test_dyn::{c_plugin_add, rust_plugin_add};

unsafe extern "C" {
    fn hello();
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> i32 {
    unsafe {
        hello();
    }
    c_plugin_add(4, rust_plugin_add(6, 7))
}
