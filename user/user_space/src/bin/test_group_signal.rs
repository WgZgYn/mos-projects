#![no_std]
#![no_main]
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::{close, exit, fork, getpid, pipe, println, read, sleep, wait4, write};
static CALLS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(_: i32) { CALLS.fetch_add(1, SeqCst); }
fn group(pid: u64, pgid: u64) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") 109u64 => result,
        in("rdi") pid, in("rsi") pgid, lateout("rcx") _, lateout("r11") _); }
    result
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[group-signal] start");
    assert_eq!(group(0, 0), 0); // isolate kill(0) from the launching shell
    let action = KernelSigAction { handler: handler as *const () as u64,
        flags: SA_RESTORER, restorer: restorer_address(), mask: 0 };
    assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
    let ready = pipe().unwrap();
    let mut children = [0u64; 3];
    for index in 0..3 {
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 {
            assert_eq!(group(0, if index == 1 { children[0] } else { 0 }), 0);
            assert_eq!(write(ready.pwrite, &[1]).unwrap(), 1);
            while CALLS.load(SeqCst) == 0 { core::hint::spin_loop(); }
            exit(11 + index as i32);
        }
        children[index] = pid;
        assert_eq!(read(ready.pread, &mut [0]).unwrap(), 1);
    }
    close(ready.pread);
    close(ready.pwrite);
    let selected = -(children[0] as i32);
    let mut status = -1;
    assert_eq!(wait4(selected, &mut status, 1), 0);
    assert_eq!(status, -1);
    assert_eq!(wait4(0, &mut status, 1), -10);
    assert_eq!(kill(selected, 0), 0);
    assert_eq!(kill(selected, SIGUSR1), 0);
    let first = wait4(selected, &mut status, 0);
    assert!(first == children[0] as i64 || first == children[1] as i64);
    assert_eq!(status, if first == children[0] as i64 { 11 << 8 } else { 12 << 8 });
    let second = wait4(selected, &mut status, 0);
    assert_ne!(first, second);
    assert!(second == children[0] as i64 || second == children[1] as i64);
    assert_eq!(status, if second == children[0] as i64 { 11 << 8 } else { 12 << 8 });
    assert_eq!(wait4(selected, &mut status, 1), -10);
    assert_eq!(CALLS.load(SeqCst), 0);
    assert_eq!(wait4(-(children[2] as i32), &mut status, 1), 0);
    assert_eq!(kill(-(children[2] as i32), SIGKILL), 0);
    assert_eq!(wait4(-(children[2] as i32), &mut status, 0), children[2] as i64);
    assert_eq!(status, SIGKILL);
    assert_eq!(kill(i32::MIN, 0), -3);
    assert_eq!(wait4(i32::MIN, &mut status, 1), -3);

    // Own-group targeting includes the sender and group-zero waiting selects
    // only its children. Child remains alive long enough to receive the signal.
    CALLS.store(0, SeqCst);
    let ready = pipe().unwrap();
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        assert_eq!(write(ready.pwrite, &[1]).unwrap(), 1);
        while CALLS.load(SeqCst) == 0 { core::hint::spin_loop(); }
        exit(21);
    }
    assert_eq!(read(ready.pread, &mut [0]).unwrap(), 1);
    assert_eq!(kill(0, SIGUSR1), 0);
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(wait4(0, &mut status, 0), pid as i64);
    assert_eq!(status, 21 << 8);
    close(ready.pread);
    close(ready.pwrite);

    // A child leaving the requested group wakes wait4 even without exiting.
    let gate = pipe().unwrap();
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        sleep(40);
        assert_eq!(group(0, 0), 0);
        assert_eq!(read(gate.pread, &mut [0]).unwrap(), 1);
        exit(0);
    }
    assert_eq!(wait4(0, &mut status, 0), -10);
    assert_eq!(write(gate.pwrite, &[1]).unwrap(), 1);
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, 0);
    close(gate.pread);
    close(gate.pwrite);
    assert!(getpid() > 0);
    println!("[group-signal] PASS");
    0
}
