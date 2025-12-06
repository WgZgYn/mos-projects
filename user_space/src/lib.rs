#![no_std]
#![no_main]

use core::arch::asm;

pub fn call_sys_write_stdout(msg: &[u8]) {
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 1,
            in("rdi") 1,
            in("rsi") msg.as_ptr(),
            in("rdx") msg.len(),
            clobber_abi("sysv64"),
        )
    }
}

pub fn call_sys_exit() -> ! {
    unsafe {
        asm!(
        "int 0x80",
        in("rax") 60,
        options(noreturn)
        )
    }
}

pub fn call_sys_rand() -> u64 {
    let ret: u64;
    unsafe {
        asm!(
        "int 0x80",
        in("rax") 7,
        lateout("rax") ret,
        )
    }
    ret
}

pub fn call_sys_yield() {
    unsafe {
        asm!(
        "int 0x80",
        in("rax") 24,
        )
    }
}