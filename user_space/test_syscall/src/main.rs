#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};

// make sure the entry point not has push pop
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!("call user_main", "ret",)
}

#[unsafe(no_mangle)]
fn user_main() {
    let msg = "Hello, World! From test_syscall, --hlt loop\n";
    unsafe {
        asm!(
        "mov rax, 1",
        "mov rdi, 1",
        "mov rsi, r8",
        "mov rdx, r9",
        "int 0x80",
        in("r8") msg.as_ptr(),
        in("r9") msg.len()
        )
    }
    loop {
        unsafe {
            asm!("nop");
        }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
