#![no_std]
#![no_main]

use core::arch::naked_asm;

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!(
        "push rax",
        "push rdi",
        "push rsi",
        "mov rax, 456",
        "mov rdi, 123",
        "mov rsi, 789",
        "int 0x80",
        "pop rsi",
        "pop rdi", 
        "pop rax",
        "ret",
    )
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
