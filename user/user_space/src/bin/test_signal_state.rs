#![no_std]
#![no_main]

use core::arch::asm;
use user_lib::signal::*;
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{execve, fork, mmap, mprotect, println, waitpid};

// Keep invalid/unaligned addresses as integers; never manufacture references.
fn raw(no: u64, args: [u64; 4]) -> i64 {
    let result;
    unsafe {
        asm!("syscall", inlateout("rax") no => result,
            in("rdi") args[0], in("rsi") args[1], in("rdx") args[2], in("r10") args[3],
            lateout("rcx") _, lateout("r11") _);
    }
    result
}

fn mask() -> u64 {
    let mut value = 0;
    assert_eq!(sigprocmask(999, None, Some(&mut value)), 0); // how ignored
    value
}

fn action(signal: i32) -> KernelSigAction {
    let mut value = KernelSigAction::default();
    assert_eq!(sigaction(signal, None, Some(&mut value)), 0);
    value
}

extern "C" fn caught_handler(_: i32) {}

#[unsafe(no_mangle)]
fn main(argc: usize) -> i32 {
    if argc == 2 {
        assert_eq!(mask(), signal_bit(SIGUSR2));
        assert_eq!(action(SIGUSR1).handler, SIG_IGN);
        assert_eq!(action(SIGUSR2), KernelSigAction::default());
        let mut pending = u64::MAX;
        assert_eq!(sigpending(&mut pending), 0);
        assert_eq!(pending, 0);
        println!("[signal-exec] PASS");
        return 0;
    }

    assert_eq!(mask(), 0);
    let all = u64::MAX;
    let mut old = 0x1234;
    assert_eq!(sigprocmask(SIG_SETMASK, Some(&all), Some(&mut old)), 0);
    assert_eq!(old, 0);
    assert_eq!(mask(), blockable(all));
    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&all), None), 0);
    let one = signal_bit(SIGUSR1);
    assert_eq!(sigprocmask(SIG_BLOCK, Some(&one), None), 0);
    assert_eq!(mask(), one);
    assert_eq!(sigprocmask(99, Some(&one), None), -22);
    assert_eq!(raw(14, [0, 0, 0, 128]), -22);
    assert_eq!(raw(14, [0, 1, 0, 8]), -14);
    assert_eq!(raw(14, [SIG_SETMASK as u64, &all as *const _ as u64, 1, 8]), -14);
    assert_eq!(mask(), one, "failed output must not change the mask");

    let mut alias = signal_bit(SIGUSR2);
    let address = &mut alias as *mut u64 as u64;
    assert_eq!(raw(14, [SIG_SETMASK as u64, address, address, 8]), 0);
    assert_eq!(alias, one);
    assert_eq!(mask(), signal_bit(SIGUSR2));

    for signal in [0, 65, SIGKILL, SIGSTOP] {
        assert_eq!(raw(13, [signal as u64, 0, 0, 8]), -22);
    }
    assert_eq!(raw(13, [SIGUSR1 as u64, 1, 0, 8]), -14);
    assert_eq!(raw(13, [SIGUSR1 as u64, 0, 0, 128]), -22);
    let mut ignored = KernelSigAction { handler: SIG_IGN, mask: all, ..Default::default() };
    assert_eq!(sigaction(SIGUSR1, Some(&ignored), None), 0);
    assert_eq!(action(SIGUSR1).mask, blockable(all));
    assert_eq!(sigaction(SIGCHLD, Some(&ignored), None), -38);
    ignored.handler = 0x1234;
    assert_eq!(sigaction(SIGUSR1, Some(&ignored), None), -22);
    assert_eq!(action(SIGUSR1).handler, SIG_IGN);
    ignored.handler = SIG_DFL;
    assert_eq!(raw(13, [SIGUSR1 as u64, &ignored as *const _ as u64, 1, 8]), -14);
    assert_eq!(action(SIGUSR1).handler, SIG_IGN);

    let mut pending = all;
    assert_eq!(sigpending(&mut pending), 0);
    assert_eq!(pending, 0);
    assert_eq!(raw(127, [1, 8, 0, 0]), -14);
    assert_eq!(raw(127, [0, 0, 0, 0]), 0);
    assert_eq!(raw(127, [0, 9, 0, 0]), -22);

    let memory = mmap(0, 8192, MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS, None, 0);
    assert!((memory as i64) > 0);
    // Untouched anonymous input reads zero; output then crosses two pages.
    assert_eq!(raw(14, [SIG_SETMASK as u64, memory + 4092, 0, 8]), 0);
    assert_eq!(mask(), 0);
    unsafe { (memory as *mut u8).add(4092).cast::<u64>().write_unaligned(one); }
    assert_eq!(raw(14, [SIG_SETMASK as u64, memory + 4092, memory + 4092, 8]), 0);
    assert_eq!(mask(), one);
    assert_eq!(unsafe { (memory as *const u8).add(4092).cast::<u64>().read_unaligned() }, 0);
    assert_eq!(mprotect(memory + 4096, 4096, 0), 0);
    assert_eq!(raw(14, [SIG_SETMASK as u64, memory + 4092, 0, 8]), -14);
    assert_eq!(mask(), one);

    // fork copies state rather than sharing it; exec preserves the child's
    // mask and ignored dispositions while clearing a caught handler.
    let default_metadata = KernelSigAction { handler: caught_handler as *const () as u64,
        flags: SA_RESTORER, restorer: restorer_address(), mask: one };
    assert_eq!(sigaction(SIGUSR2, Some(&default_metadata), None), 0);
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        assert_eq!(mask(), one);
        assert_eq!(action(SIGUSR1).handler, SIG_IGN);
        assert_eq!(action(SIGUSR2), default_metadata);
        assert_eq!(sigprocmask(SIG_SETMASK, Some(&signal_bit(SIGUSR2)), None), 0);
        let args = [c"/test_signal_state".as_ptr(), c"exec".as_ptr(), core::ptr::null()];
        execve(c"/test_signal_state", &args, &[core::ptr::null()]).unwrap();
        unreachable!();
    }
    assert_eq!(waitpid(pid), 0);
    assert_eq!(mask(), one);
    assert_eq!(action(SIGUSR2), default_metadata);
    println!("[signal-state] PASS");
    0
}
