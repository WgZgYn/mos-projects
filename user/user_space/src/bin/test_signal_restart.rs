#![no_std]
#![no_main]

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::mos::signal_frame::{MCONTEXT, UCONTEXT};
use user_lib::signal::*;
use user_lib::{close, exit, fork, getpid, pipe, println, read, sleep, wait4, write};
use user_lib::mos::fs::FileDesc;

static MODE: AtomicUsize = AtomicUsize::new(0);
static ACK: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static NESTED: AtomicUsize = AtomicUsize::new(0);
static STATUS: AtomicUsize = AtomicUsize::new(0);

extern "C" fn nested(_: i32) { NESTED.fetch_add(1, SeqCst); }

extern "C" fn handler(_: i32, _: *const u8, context: *mut u8) {
    let mode = MODE.load(SeqCst);
    let restart = mode & 1 != 0;
    unsafe {
        let regs = context.add(MCONTEXT - UCONTEXT).cast::<u64>();
        let rax = regs.add(13).read_unaligned();
        let rip = regs.add(16).read_unaligned();
        assert_eq!(rax as i64, if restart { 61 } else { -4 });
        let opcode = (if restart { rip } else { rip - 2 }) as *const u8;
        let expected = if mode & 2 != 0 { [0xcd, 0x80] } else { [0x0f, 0x05] };
        assert_eq!([opcode.read(), opcode.add(1).read()], expected);
        assert_eq!((STATUS.load(SeqCst) as *const i32).read_volatile(), 0x1234);
        if mode & 4 != 0 {
            // A handler owns its ucontext: it can intentionally cancel the
            // prepared restart and synthesize a result for the interrupted call.
            regs.add(13).write_unaligned(123);
            regs.add(16).write_unaligned(rip + 2);
        }
    }
    let mut status = 0;
    assert_eq!(wait4(i32::MAX, &mut status, 0), -10);
    assert_eq!(raise(SIGUSR2), 0);
    assert_eq!(NESTED.load(SeqCst), 1);
    CALLS.fetch_add(1, SeqCst);
    assert_eq!(write(FileDesc(ACK.load(SeqCst)), &[1]).unwrap(), 1);
}

#[inline(never)]
fn raw_wait(pid: i32, status: &mut i32, int80: bool) -> i64 {
    let result: i64;
    unsafe {
        if int80 {
            // MOS's existing int80 entry uses its x86_64 argument convention.
            asm!("int 0x80", inlateout("rax") 61u64 => result,
                in("rdi") pid as i64, in("rsi") status as *mut i32,
                in("rdx") 0u64, in("r10") 0u64,
                lateout("rcx") _, lateout("r11") _);
        } else {
            asm!("syscall", inlateout("rax") 61u64 => result,
                in("rdi") pid as i64, in("rsi") status as *mut i32,
                in("rdx") 0u64, in("r10") 0u64,
                lateout("rcx") _, lateout("r11") _);
        }
    }
    result
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[signal-restart] start");
    let parent = getpid() as i32;
    for mode in [0, 1, 2, 3, 5, 7] {
        MODE.store(mode, SeqCst);
        CALLS.store(0, SeqCst);
        NESTED.store(0, SeqCst);
        let action = KernelSigAction { handler: handler as *const () as u64,
            flags: SA_RESTORER | SA_SIGINFO | if mode & 1 != 0 { SA_RESTART } else { 0 },
            restorer: restorer_address(), mask: 0 };
        assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
        let action = KernelSigAction { handler: nested as *const () as u64,
            flags: SA_RESTORER, restorer: restorer_address(), mask: 0 };
        assert_eq!(sigaction(SIGUSR2, Some(&action), None), 0);
        let ack = pipe().unwrap();
        ACK.store(ack.pwrite.0, SeqCst);
        let pid = fork();
        assert!((pid as i64) >= 0);
        if pid == 0 {
            close(ack.pwrite);
            sleep(50);
            assert_eq!(kill(parent, SIGUSR1), 0);
            let mut byte = [0];
            // Do not exit until the handler has inspected the interrupted
            // frame: a normal child exit cannot masquerade as a restart pass.
            assert_eq!(read(ack.pread, &mut byte).unwrap(), 1);
            assert_eq!(byte, [1]);
            close(ack.pread);
            sleep(20);
            exit(41);
        }
        close(ack.pread);
        let mut status = 0x1234;
        STATUS.store(&mut status as *mut i32 as usize, SeqCst);
        let result = raw_wait(pid as i32, &mut status, mode & 2 != 0);
        assert_eq!(CALLS.load(SeqCst), 1);
        if mode & 1 == 0 || mode & 4 != 0 {
            assert_eq!(result, if mode & 4 != 0 { 123 } else { -4 });
            assert_eq!(status, 0x1234);
            assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
        } else {
            assert_eq!(result, pid as i64);
        }
        assert_eq!(status, 41 << 8);
        assert_eq!(wait4(pid as i32, &mut status, 0), -10);
        close(ack.pwrite);
    }
    println!("[signal-restart] PASS");
    0
}
