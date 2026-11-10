#![no_std]
#![no_main]
use core::arch::asm;
use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::{exit, fork, println, sleep, wait4};
static CALLS: AtomicUsize = AtomicUsize::new(0);
static PID: AtomicI32 = AtomicI32::new(0);
static CODE: AtomicI32 = AtomicI32::new(0);
static STATUS: AtomicI32 = AtomicI32::new(0);
static REAP: AtomicUsize = AtomicUsize::new(0);
extern "C" fn handler(signal: i32, info: *const u8, _: *mut u8) {
    assert_eq!(signal, SIGCHLD);
    unsafe {
        PID.store(info.add(16).cast::<i32>().read_unaligned(), SeqCst);
        CODE.store(info.add(8).cast::<i32>().read_unaligned(), SeqCst);
        STATUS.store(info.add(24).cast::<i32>().read_unaligned(), SeqCst);
    }
    if REAP.load(SeqCst) != 0 {
        let mut status = -1;
        // Completion must be visible by the time the handler runs.
        assert_eq!(wait4(PID.load(SeqCst), &mut status, 1), PID.load(SeqCst) as i64);
        assert_eq!(status, STATUS.load(SeqCst) << 8);
    }
    CALLS.fetch_add(1, SeqCst);
}
fn reap(pid: u64, expected: i32) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, expected);
}
fn pause() -> i64 {
    let ts = [5i64, 0];
    let result;
    unsafe { asm!("syscall", inlateout("rax") 35u64 => result,
        in("rdi") ts.as_ptr(), in("rsi") 0u64,
        lateout("rcx") _, lateout("r11") _); }
    result
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[sigchld] start");
    let action = KernelSigAction { handler: handler as *const () as u64,
        flags: SA_RESTORER | SA_SIGINFO, restorer: restorer_address(), mask: 0 };
    assert_eq!(sigaction(SIGCHLD, Some(&action), None), 0);
    for handler_reaps in [false, true] {
        CALLS.store(0, SeqCst);
        REAP.store(handler_reaps as usize, SeqCst);
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 { sleep(40); exit(37); }
        assert_eq!(pause(), -4);
        assert_eq!(CALLS.load(SeqCst), 1);
        assert_eq!(PID.load(SeqCst), pid as i32);
        assert_eq!(CODE.load(SeqCst), 1); // CLD_EXITED
        assert_eq!(STATUS.load(SeqCst), 37);
        if !handler_reaps { reap(pid, 37 << 8); }
        let mut status = -1;
        assert_eq!(wait4(pid as i32, &mut status, 1), -10);
    }
    REAP.store(0, SeqCst);
    CALLS.store(0, SeqCst);
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 { loop { core::hint::spin_loop(); } }
    assert_eq!(kill(pid as i32, SIGKILL), 0);
    while CALLS.load(SeqCst) == 0 { core::hint::spin_loop(); }
    assert_eq!(CODE.load(SeqCst), 2); // CLD_KILLED
    assert_eq!(STATUS.load(SeqCst), SIGKILL);
    assert_eq!(PID.load(SeqCst), pid as i32);
    reap(pid, SIGKILL);

    // Masked standard notifications coalesce, independent wait records do not.
    CALLS.store(0, SeqCst);
    let bit = signal_bit(SIGCHLD);
    assert_eq!(sigprocmask(SIG_BLOCK, Some(&bit), None), 0);
    let first = fork();
    assert!((first as i64) >= 0);
    if first == 0 { exit(11); }
    reap(first, 11 << 8);
    let second = fork();
    assert!((second as i64) >= 0);
    if second == 0 { exit(22); }
    reap(second, 22 << 8);
    let mut pending = 0;
    assert_eq!(sigpending(&mut pending), 0);
    assert_ne!(pending & bit, 0);
    assert_eq!(CALLS.load(SeqCst), 0);
    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&bit), None), 0);
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(PID.load(SeqCst), first as i32);
    assert_eq!(STATUS.load(SeqCst), 11);
    println!("[sigchld] PASS");
    0
}
