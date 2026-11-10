#![no_std]
#![no_main]

use core::arch::asm;
use user_lib::{exit, fork, println, sleep, wait4, yield_};

fn set_fs(address: u64) -> i64 {
    let result;
    unsafe {
        asm!("syscall", inlateout("rax") 158u64 => result,
            in("rdi") 0x1002u64, in("rsi") address,
            lateout("rcx") _, lateout("r11") _);
    }
    result
}
fn fs_word() -> u64 {
    let word;
    unsafe { asm!("mov {}, fs:[0]", out(reg) word, options(nostack, readonly, preserves_flags)); }
    word
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[fork-tls] start");
    let mut slot = 0x1234_5678u64;
    assert_eq!(set_fs(&mut slot as *mut u64 as u64), 0);
    assert_eq!(set_fs(u64::MAX), -22);
    for round in 0..8 {
        let pid = fork();
        assert!((pid as i64) >= 0);
        // Child's first return must restore inherited TLS, even though it
        // never passed through a previous Rust context_switch invocation.
        assert_eq!(fs_word(), 0x1234_5678);
        if pid == 0 {
            let own = 0xabcdef00 + round;
            unsafe { (&mut slot as *mut u64).write_volatile(own); }
            for _ in 0..12 {
                yield_();
                assert_eq!(fs_word(), own);
                sleep(1);
                assert_eq!(fs_word(), own);
            }
            assert_eq!(set_fs(0), 0);
            exit(27);
        }
        for _ in 0..12 {
            yield_();
            assert_eq!(fs_word(), 0x1234_5678);
        }
        let mut status = 0;
        assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
        assert_eq!(status, 27 << 8);
        assert_eq!(fs_word(), 0x1234_5678);
    }
    assert_eq!(set_fs(0), 0);
    println!("[fork-tls] PASS");
    0
}
