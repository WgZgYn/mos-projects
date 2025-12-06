#![no_std]
#![no_main]

use bs_string::BString;
use core::arch::naked_asm;
use user_space::{call_sys_exit, call_sys_rand, call_sys_write_stdout, call_sys_yield};

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!("jmp user_main")
}

#[unsafe(no_mangle)]
fn user_main() {
    let mut r = call_sys_rand();
    call_sys_write_stdout(b"----- test_rand ----- start");
    let mut s = BString::<32>::new();
    while r > 0 {
        s.push_byte((r % 10) as u8 + b'0');
        r /= 10;
    }
    s.reverse();

    call_sys_write_stdout(b"--- call yield ---");
    call_sys_yield();

    call_sys_write_stdout(s.as_bytes());
    call_sys_write_stdout(b"----- test_rand ----- end");
    call_sys_exit()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
