#![no_std]
#![no_main]

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::mos::io::Iovec;
use user_lib::signal::*;
use user_lib::{close, execve, exit, fork, getpid, pipe, println, sleep, wait4};

static CALLS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(signal: i32, info: *const u8, _: *mut u8) {
    assert_eq!(signal, SIGPIPE);
    unsafe {
        assert_eq!(info.add(8).cast::<i32>().read_unaligned(), 0);
        assert_eq!(info.add(16).cast::<i32>().read_unaligned(), getpid() as i32);
    }
    CALLS.fetch_add(1, SeqCst);
}
fn raw(number: u64, fd: usize, address: u64, count: usize) -> i64 {
    let result;
    unsafe {
        asm!("syscall", inlateout("rax") number => result,
            in("rdi") fd, in("rsi") address, in("rdx") count,
            lateout("rcx") _, lateout("r11") _);
    }
    result
}
fn install() {
    let action = KernelSigAction { handler: handler as *const () as u64,
        flags: SA_RESTORER | SA_SIGINFO, restorer: restorer_address(), mask: 0 };
    assert_eq!(sigaction(SIGPIPE, Some(&action), None), 0);
    CALLS.store(0, SeqCst);
}
fn reap(pid: u64, expected: i32) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, expected);
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[sigpipe] start");
    for vectored in [false, true] {
        for mode in 0..4 {
            let pid = fork();
            assert!((pid as i64) >= 0);
            if pid == 0 {
                let channel = pipe().unwrap();
                close(channel.pread);
                if mode == 1 {
                    let action = KernelSigAction { handler: SIG_IGN, ..KernelSigAction::default() };
                    assert_eq!(sigaction(SIGPIPE, Some(&action), None), 0);
                }
                if mode >= 2 { install(); }
                let mask = signal_bit(SIGPIPE);
                if mode == 3 { assert_eq!(sigprocmask(SIG_BLOCK, Some(&mask), None), 0); }
                // Zero transfers neither dereference the pointer nor signal.
                assert_eq!(raw(1, channel.pwrite.0, 0, 0), 0);
                assert_eq!(raw(20, channel.pwrite.0, 0, 0), 0);
                let data = [b'x'];
                let iov = [Iovec { iov_base: data.as_ptr() as u64, iov_len: 1 }];
                let write_once = || if vectored {
                    raw(20, channel.pwrite.0, iov.as_ptr() as u64, 1)
                } else { raw(1, channel.pwrite.0, data.as_ptr() as u64, 1) };
                assert_eq!(write_once(), -32);
                assert!(mode != 0, "default SIGPIPE must terminate before returning");
                if mode == 2 { assert_eq!(CALLS.load(SeqCst), 1); }
                if mode == 3 {
                    assert_eq!(write_once(), -32);
                    assert_eq!(CALLS.load(SeqCst), 0);
                    let mut pending = 0;
                    assert_eq!(sigpending(&mut pending), 0);
                    assert_eq!(pending & mask, mask);
                    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&mask), None), 0);
                    assert_eq!(CALLS.load(SeqCst), 1);
                }
                close(channel.pwrite);
                exit(0);
            }
            reap(pid, if mode == 0 { SIGPIPE } else { 0 });
        }
    }

    // First iovec fills this pipe; the second waits until the last reader
    // closes. The first 512 bytes remain a successful partial write.
    install();
    let channel = pipe().unwrap();
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        close(channel.pwrite);
        sleep(100);
        close(channel.pread);
        exit(0);
    }
    close(channel.pread);
    let data = [b'x'; 512];
    let iov = [Iovec { iov_base: data.as_ptr() as u64, iov_len: 512 },
        Iovec { iov_base: data.as_ptr() as u64, iov_len: 1 }];
    assert_eq!(raw(20, channel.pwrite.0, iov.as_ptr() as u64, 2), 512);
    assert_eq!(CALLS.load(SeqCst), 1);
    close(channel.pwrite);
    reap(pid, 0);

    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        let args = [c"/busybox".as_ptr(), c"sh".as_ptr(), c"-c".as_ptr(),
            c"/busybox yes pipe_signal_line | /busybox head -n 1".as_ptr(), core::ptr::null()];
        execve(c"/busybox", &args, &[c"PATH=/".as_ptr(), core::ptr::null()]).unwrap();
        unreachable!();
    }
    reap(pid, 0);
    println!("[sigpipe] PASS");
    0
}
