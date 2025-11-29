#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!("call user_main", "ret",)
}

#[unsafe(no_mangle)]
fn user_main() -> u64 {
    let r;
    unsafe {
        asm!(
        "int 0x80",
        in("rax") 7,
        lateout("rax") r,
        )
    }
    r
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}