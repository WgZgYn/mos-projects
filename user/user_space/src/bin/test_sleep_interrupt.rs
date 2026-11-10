#![no_std]
#![no_main]
use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::{exit, fork, getpid, println, sleep, wait4};
static CALLS: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(_: i32) { CALLS.fetch_add(1, SeqCst); }
fn raw(nr: u64, a: u64, b: u64) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") nr => result, in("rdi") a,
        in("rsi") b, lateout("rcx") _, lateout("r11") _); }
    result
}
fn now() -> i64 {
    let mut ts = [0i64; 2];
    assert_eq!(raw(228, 1, ts.as_mut_ptr() as u64), 0);
    ts[0] * 1000 + ts[1] / 1_000_000
}
fn reap(pid: u64, expected: i32) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, expected);
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[sleep-interrupt] start");
    assert_eq!(raw(35, 0, 0), -14);
    assert_eq!(raw(35, 0xffff_8000_0000_0000, 0), -14);
    for invalid in [[-1i64, 0], [0, -1], [0, 1_000_000_000]] {
        assert_eq!(raw(35, invalid.as_ptr() as u64, 0), -22);
    }
    let zero = [0i64; 2];
    // rem is only accessed for interruption, never on successful completion.
    assert_eq!(raw(35, zero.as_ptr() as u64, 1), 0);
    let parent = getpid() as i32;
    for restart in [false, true] {
        let action = KernelSigAction { handler: handler as *const () as u64,
            flags: SA_RESTORER | if restart { SA_RESTART } else { 0 },
            restorer: restorer_address(), mask: 0 };
        assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
        for bad_rem in [false, true] {
            CALLS.store(0, SeqCst);
            let pid = fork();
            assert!((pid as i64) >= 0);
            if pid == 0 {
                sleep(40);
                assert_eq!(kill(parent, SIGUSR1), 0);
                exit(0);
            }
            let mut ts = [0i64, 250_000_000];
            let ptr = ts.as_mut_ptr() as u64;
            // Alias req/rem to prove input was snapshotted before output.
            assert_eq!(raw(35, ptr, if bad_rem { 1 } else { ptr }),
                if bad_rem { -14 } else { -4 });
            assert_eq!(CALLS.load(SeqCst), 1);
            if !bad_rem {
                assert_eq!(ts[0], 0);
                assert!(ts[1] > 0 && ts[1] <= 250_000_000);
            }
            reap(pid, 0);
            // Cross the cancelled timer's old deadline during a later sleep.
            // A stale TCB wake must not make this syscall return early.
            let later = [0i64, 350_000_000];
            let start = now();
            assert_eq!(raw(35, later.as_ptr() as u64, 0), 0);
            assert!(now() - start >= 350);
        }
    }
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        let long = [30i64, 0];
        raw(35, long.as_ptr() as u64, 0);
        panic!("SIGKILL must terminate nanosleep");
    }
    sleep(40);
    assert_eq!(kill(pid as i32, SIGKILL), 0);
    reap(pid, SIGKILL);
    println!("[sleep-interrupt] PASS");
    0
}
