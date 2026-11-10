#![no_std]
#![no_main]
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::mos::fs::FileDesc;
use user_lib::{close, exit, fork, getpid, pipe, println, read, sleep, wait4, write};
static ACK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static RESTART: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(_: i32) {
    CALLS.fetch_add(1, SeqCst);
    assert_eq!(write(FileDesc(ACK.load(SeqCst)), &[1]).unwrap(), 1);
    if RESTART.load(SeqCst) != 0 { println!("[tty-interrupt] restart-key"); }
}
fn tty_read(byte: &mut u8) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") 0u64 => result,
        in("rdi") 0u64, in("rsi") byte as *mut u8, in("rdx") 1u64,
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
    println!("[tty-interrupt] start");
    let parent = getpid() as i32;
    for restart in [false, false, false, true] {
        RESTART.store(restart as usize, SeqCst);
        CALLS.store(0, SeqCst);
        let action = KernelSigAction { handler: handler as *const () as u64,
            flags: SA_RESTORER | if restart { SA_RESTART } else { 0 },
            restorer: restorer_address(), mask: 0 };
        assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
        let ack = pipe().unwrap();
        ACK.store(ack.pwrite.0, SeqCst);
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 {
            close(ack.pwrite);
            sleep(40);
            assert_eq!(kill(parent, SIGUSR1), 0);
            assert_eq!(read(ack.pread, &mut [0]).unwrap(), 1);
            close(ack.pread);
            exit(0);
        }
        close(ack.pread);
        let mut byte = b'X';
        let result = tty_read(&mut byte);
        assert_eq!(CALLS.load(SeqCst), 1);
        if restart {
            // The runner supplies q + Enter only after the handler marker.
            assert_eq!(result, 1);
            assert_eq!(byte, b'q');
            assert_eq!(tty_read(&mut byte), 1);
            assert_eq!(byte, b'\n');
        } else {
            assert_eq!(result, -4);
            assert_eq!(byte, b'X');
        }
        reap(pid, 0);
        close(ack.pwrite);
    }
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        tty_read(&mut 0);
        panic!("SIGKILL must terminate TTY read");
    }
    sleep(40);
    assert_eq!(kill(pid as i32, SIGKILL), 0);
    reap(pid, SIGKILL);
    println!("[tty-interrupt] buffered-key");
    sleep(800);
    let mut byte = 0;
    assert_eq!(tty_read(&mut byte), 1);
    assert_eq!(byte, b'z');
    assert_eq!(tty_read(&mut byte), 1);
    assert_eq!(byte, b'\n');
    println!("[tty-interrupt] PASS");
    0
}
