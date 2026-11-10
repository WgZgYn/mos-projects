#![no_std]
#![no_main]
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::mos::fs::FileDesc;
use user_lib::{close, exit, fork, getpid, pipe, println, read, sleep, wait4, write};
static ACK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(_: i32) {
    CALLS.fetch_add(1, SeqCst);
    assert_eq!(write(FileDesc(ACK.load(SeqCst)), &[1]).unwrap(), 1);
}
fn raw(number: u64, fd: usize, pointer: u64, length: u64) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") number => result,
        in("rdi") fd, in("rsi") pointer, in("rdx") length,
        lateout("rcx") _, lateout("r11") _); }
    result
}
fn reap(pid: u64, expected: i32) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, expected);
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[pipe-interrupt] start");
    let parent = getpid() as i32;
    for writing in [false, true] {
        for restart in [false, true] {
            for _ in 0..3 {
                CALLS.store(0, SeqCst);
                let action = KernelSigAction { handler: handler as *const () as u64,
                    flags: SA_RESTORER | if restart { SA_RESTART } else { 0 },
                    restorer: restorer_address(), mask: 0 };
                assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
                let data = pipe().unwrap();
                let ack = pipe().unwrap();
                ACK.store(ack.pwrite.0, SeqCst);
                if writing { assert_eq!(write(data.pwrite, &[0x55; 512]).unwrap(), 512); }
                let pid = fork();
                assert!((pid as i64) >= 0);
                if pid == 0 {
                    close(ack.pwrite);
                    sleep(40);
                    assert_eq!(kill(parent, SIGUSR1), 0);
                    let mut byte = [0];
                    // Parent must run the handler BEFORE we provide data/space.
                    assert_eq!(read(ack.pread, &mut byte).unwrap(), 1);
                    if writing { assert_eq!(read(data.pread, &mut byte).unwrap(), 1); }
                    else { assert_eq!(write(data.pwrite, b"Q").unwrap(), 1); }
                    close(ack.pread);
                    close(data.pread);
                    close(data.pwrite);
                    exit(0);
                }
                close(ack.pread);
                let mut byte = [b'X'];
                let number = if writing { 1 } else { 0 };
                let fd = if writing { data.pwrite.0 } else { data.pread.0 };
                let result = raw(number, fd, byte.as_mut_ptr() as u64, 1);
                assert_eq!(CALLS.load(SeqCst), 1);
                if restart { assert_eq!(result, 1); }
                else {
                    assert_eq!(result, -4);
                    assert_eq!(byte, [b'X']);
                    assert_eq!(raw(number, fd, byte.as_mut_ptr() as u64, 1), 1);
                }
                if !writing { assert_eq!(byte, [b'Q']); }
                reap(pid, 0);
                close(ack.pwrite);
                close(data.pread);
                close(data.pwrite);
            }
        }
    }
    // Termination must also reach a child blocked on an otherwise live pipe.
    let data = pipe().unwrap();
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        let mut byte = [0];
        read(data.pread, &mut byte).unwrap();
        panic!("blocked read should terminate via SIGKILL");
    }
    sleep(40);
    assert_eq!(kill(pid as i32, SIGKILL), 0);
    reap(pid, SIGKILL);
    close(data.pread);
    close(data.pwrite);
    println!("[pipe-interrupt] PASS");
    0
}
