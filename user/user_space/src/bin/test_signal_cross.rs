#![no_std]
#![no_main]

use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::{close, exit, fork, getpid, pipe, println, read, sleep, wait4, write};

static CALLS: AtomicUsize = AtomicUsize::new(0);
static SENDER: AtomicI32 = AtomicI32::new(0);
static CODE: AtomicI32 = AtomicI32::new(0);

extern "C" fn handler(signal: i32, info: *const u8, _: *mut u8) {
    assert_eq!(signal, SIGUSR1);
    unsafe {
        SENDER.store(info.add(16).cast::<i32>().read_unaligned(), SeqCst);
        CODE.store(info.add(8).cast::<i32>().read_unaligned(), SeqCst);
    }
    CALLS.fetch_add(1, SeqCst);
}

fn install() {
    let action = KernelSigAction { handler: handler as *const () as u64,
        flags: SA_RESTORER | SA_SIGINFO, restorer: restorer_address(), mask: 0 };
    assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
    CALLS.store(0, SeqCst);
}

fn reap(pid: u64, expected: i32) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, expected);
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[signal-cross] start");
    install();
    assert_eq!(kill(i32::MAX, 0), -3); // ESRCH, including signal-zero probes
    assert_eq!(kill(i32::MAX, 65), -22);
    assert_eq!(kill(0, 0), 0); // probe own group without signaling the parent shell
    assert_eq!(tkill(0, 0), -22);

    // Send to a child spinning entirely in userspace: only timer-return
    // delivery can run its handler. A pipe reports its actual TID first.
    let parent = getpid() as i32;
    for directed in [false, true] {
        install();
        let channel = pipe().unwrap();
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 {
            close(channel.pread);
            let tid = gettid();
            assert_eq!(write(channel.pwrite, &tid.to_ne_bytes()).unwrap(), 4);
            close(channel.pwrite);
            while CALLS.load(SeqCst) == 0 { core::hint::spin_loop(); }
            assert_eq!(SENDER.load(SeqCst), parent);
            assert_eq!(CODE.load(SeqCst), if directed { -6 } else { 0 });
            exit(37);
        }
        close(channel.pwrite);
        let mut bytes = [0; 4];
        assert_eq!(read(channel.pread, &mut bytes).unwrap(), 4);
        close(channel.pread);
        let id = if directed { i32::from_ne_bytes(bytes) } else { pid as i32 };
        assert_eq!(if directed { tkill(id, 0) } else { kill(id, 0) }, 0);
        assert_eq!(if directed { tkill(id, SIGUSR1) } else { kill(id, SIGUSR1) }, 0);
        reap(pid, 37 << 8);
        assert_eq!(kill(pid as i32, 0), -3);
    }

    // Child stays alive after sending, so successful wait4 cannot hide a lost
    // signal wakeup. Repeat both immediate and delayed sends around park.
    for delay in [0, 30, 0, 30, 0, 30, 0, 30] {
        install();
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 {
            if delay != 0 { sleep(delay); }
            assert_eq!(kill(parent, SIGUSR1), 0);
            sleep(200);
            exit(19);
        }
        let mut status = 0x1234;
        let result = wait4(pid as i32, &mut status, 0);
        // An immediate send can be delivered before entering wait4. Delayed
        // sends exercise a parked waiter; neither path may lose child status.
        if result == -4 {
            assert_eq!(status, 0x1234);
            reap(pid, 19 << 8);
        } else {
            assert_eq!(delay, 0);
            assert_eq!(result, pid as i64);
            assert_eq!(status, 19 << 8);
        }
        assert_eq!(CALLS.load(SeqCst), 1);
        assert_eq!(SENDER.load(SeqCst), pid as i32);
    }

    // Blocked standard signals coalesce and do not interrupt wait4. The first
    // sender metadata remains valid even after that child has been reaped.
    install();
    let mask = signal_bit(SIGUSR1);
    assert_eq!(sigprocmask(SIG_BLOCK, Some(&mask), None), 0);
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        assert_eq!(kill(parent, SIGUSR1), 0);
        assert_eq!(kill(parent, SIGUSR1), 0);
        exit(23);
    }
    reap(pid, 23 << 8);
    assert_eq!(CALLS.load(SeqCst), 0);
    let mut pending = 0;
    assert_eq!(sigpending(&mut pending), 0);
    assert_eq!(pending & mask, mask);
    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&mask), None), 0);
    assert_eq!(CALLS.load(SeqCst), 1);
    assert_eq!(SENDER.load(SeqCst), pid as i32);

    let ignored = KernelSigAction { handler: SIG_IGN, ..KernelSigAction::default() };
    assert_eq!(sigaction(SIGUSR1, Some(&ignored), None), 0);
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        sleep(30);
        assert_eq!(kill(parent, SIGUSR1), 0);
        sleep(30);
        exit(29);
    }
    reap(pid, 29 << 8); // ignored signals must not spuriously interrupt wait
    assert_eq!(CALLS.load(SeqCst), 1);

    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 { loop { core::hint::spin_loop(); } }
    assert_eq!(kill(pid as i32, SIGKILL), 0);
    reap(pid, SIGKILL);
    println!("[signal-cross] PASS");
    0
}
