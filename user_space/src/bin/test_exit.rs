#![no_std]
#![no_main]

use core::arch::naked_asm;
use user_space::{call_sys_exit, call_sys_write_stdout};

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!("jmp user_main")
}

#[unsafe(no_mangle)]
fn user_main() -> ! {
    call_sys_write_stdout(b"----- test_exit -----");
    call_sys_exit()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
