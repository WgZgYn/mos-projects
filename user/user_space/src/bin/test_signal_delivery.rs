#![no_std]
#![no_main]

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering::SeqCst};
use user_lib::signal::*;
use user_lib::mos::signal_frame::{MCONTEXT, UCONTEXT, FP_POINTER};
use user_lib::{fork, getpid, println, wait4};

static MODE: AtomicUsize = AtomicUsize::new(0);
static CALLS: AtomicUsize = AtomicUsize::new(0);
static DEPTH: AtomicUsize = AtomicUsize::new(0);
static MAX_DEPTH: AtomicUsize = AtomicUsize::new(0);

extern "C" fn handler(signal: i32, info: *const u8, context: *mut u8) {
    let n = CALLS.fetch_add(1, SeqCst);
    let depth = DEPTH.fetch_add(1, SeqCst) + 1;
    MAX_DEPTH.fetch_max(depth, SeqCst);
    assert_eq!(signal, SIGUSR1);
    assert_eq!(unsafe { info.cast::<i32>().read_unaligned() }, SIGUSR1);
    let mode = MODE.load(SeqCst);
    if mode == 0 {
        assert_eq!(unsafe { info.add(8).cast::<i32>().read_unaligned() }, 0); // SI_USER
        let mut mask = 0;
        assert_eq!(sigprocmask(0, None, Some(&mut mask)), 0);
        assert_eq!(mask & (signal_bit(SIGUSR1) | signal_bit(SIGUSR2)), signal_bit(SIGUSR1) | signal_bit(SIGUSR2));
        unsafe {
            let regs = context.add(MCONTEXT - UCONTEXT).cast::<u64>();
            regs.add(13).write_unaligned(77); // edited RAX must survive sigreturn
            regs.add(14).write_unaligned(0x13579); // RCX differs from return RIP
            regs.add(3).write_unaligned(0x2468); // R11 differs from RFLAGS
            let flags = regs.add(17).read_unaligned();
            regs.add(17).write_unaligned(flags | (3 << 12) | (1 << 14) | (1 << 17));
            asm!("pxor xmm15, xmm15", out("xmm15") _, options(nostack));
        }
    } else if (mode == 1 || mode == 2) && n == 0 {
        assert_eq!(raise(SIGUSR1), 0);
        if mode == 1 { assert_eq!(CALLS.load(SeqCst), 1); }
        else { assert_eq!(CALLS.load(SeqCst), 2); }
    } else if mode == 3 {
        let mut action = KernelSigAction::default();
        assert_eq!(sigaction(SIGUSR1, None, Some(&mut action)), 0);
        assert_eq!(action.handler, SIG_DFL);
    } else if mode >= 10 {
        unsafe {
            let regs = context.add(MCONTEXT - UCONTEXT).cast::<u64>();
            match mode {
                10 => regs.add(16).write_unaligned(0xffff_8000_0000_0000),
                11 => regs.add(18).write_unaligned(0x10_0000_0000_0008),
                12 => {
                    let fp = context.add(FP_POINTER - UCONTEXT).cast::<u64>().read_unaligned();
                    (fp as *mut u8).add(24).cast::<u32>().write_unaligned(u32::MAX);
                }
                13 => context.add(FP_POINTER - UCONTEXT).cast::<u64>().write_unaligned(1),
                14 => regs.add(15).write_unaligned(u64::MAX),
                _ => unreachable!(),
            }
        }
    }
    DEPTH.fetch_sub(1, SeqCst);
}

fn install(mode: usize, flags: u64) {
    MODE.store(mode, SeqCst);
    CALLS.store(0, SeqCst);
    DEPTH.store(0, SeqCst);
    MAX_DEPTH.store(0, SeqCst);
    let action = KernelSigAction { handler: handler as *const () as u64,
        restorer: restorer_address(), flags: SA_RESTORER | SA_SIGINFO | flags,
        mask: if mode == 0 { signal_bit(SIGUSR2) } else { 0 } };
    assert_eq!(sigaction(SIGUSR1, Some(&action), None), 0);
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let invalid = KernelSigAction { handler: handler as *const () as u64, ..Default::default() };
    assert_eq!(sigaction(SIGUSR1, Some(&invalid), None), -22);
    assert_eq!(kill(getpid() as i32, 0), 0);
    assert_eq!(kill(getpid() as i32, 32), -38);
    install(0, 0);
    let seed = [0x5au8; 16];
    let mut output = [0u8; 16];
    let (result, rcx, r11, r12, r8, redzone, flags): (u64, u64, u64, u64, u64, u64, u64);
    unsafe {
        asm!(
            "sub rsp, 128",
            "mov qword ptr [rsp - 64], 0x123456",
            "mov r12, 0x1234",
            "mov r8, 0x8877",
            "movdqu xmm15, [{seed}]",
            "syscall",
            "movdqu [{output}], xmm15",
            "mov r9, [rsp - 64]",
            "pushfq", "pop r10",
            "add rsp, 128",
            seed = in(reg) seed.as_ptr(), output = in(reg) output.as_mut_ptr(),
            inlateout("rax") 62u64 => result, in("rdi") getpid(), in("rsi") SIGUSR1 as u64,
            out("rcx") rcx, out("r11") r11, out("r12") r12,
            out("r8") r8, out("r9") redzone, out("r10") flags,
            out("xmm15") _,
        );
    }
    assert_eq!((result, rcx, r11), (77, 0x13579, 0x2468));
    assert_eq!((r12, r8, redzone), (0x1234, 0x8877, 0x123456));
    assert_eq!(flags & (3 << 12 | 1 << 14 | 1 << 17), 0);
    assert_eq!(output, seed);
    assert_eq!(CALLS.load(SeqCst), 1);
    let mut mask = 1;
    assert_eq!(sigprocmask(0, None, Some(&mut mask)), 0);
    assert_eq!(mask, 0);

    install(4, 0);
    let bit = signal_bit(SIGUSR1);
    assert_eq!(sigprocmask(SIG_BLOCK, Some(&bit), None), 0);
    assert_eq!(raise(SIGUSR1), 0);
    assert_eq!(raise(SIGUSR1), 0); // standard pending signals coalesce
    assert_eq!(CALLS.load(SeqCst), 0);
    assert_eq!(sigpending(&mut mask), 0);
    assert_eq!(mask, bit);
    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&bit), None), 0);
    assert_eq!(CALLS.load(SeqCst), 1);

    install(4, 0);
    assert_eq!(sigprocmask(SIG_BLOCK, Some(&bit), None), 0);
    assert_eq!(raise(SIGUSR1), 0);
    let ignore = KernelSigAction { handler: SIG_IGN, ..Default::default() };
    assert_eq!(sigaction(SIGUSR1, Some(&ignore), None), 0);
    assert_eq!(sigpending(&mut mask), 0);
    assert_eq!(mask, 0);
    assert_eq!(sigprocmask(SIG_UNBLOCK, Some(&bit), None), 0);
    assert_eq!(CALLS.load(SeqCst), 0);

    install(1, 0);
    assert_eq!(raise(SIGUSR1), 0);
    assert_eq!(CALLS.load(SeqCst), 2);
    assert_eq!(MAX_DEPTH.load(SeqCst), 1);
    install(2, SA_NODEFER);
    assert_eq!(raise(SIGUSR1), 0);
    assert_eq!(CALLS.load(SeqCst), 2);
    assert_eq!(MAX_DEPTH.load(SeqCst), 2);
    install(3, SA_RESETHAND);
    assert_eq!(raise(SIGUSR1), 0);
    assert_eq!(CALLS.load(SeqCst), 1);

    for mode in 10..=16 {
        let pid = fork();
        assert!((pid as i64) > 0 || pid == 0);
        if pid == 0 {
            if mode == 15 {
                unsafe { asm!("syscall", inlateout("rax") 15u64 => _, lateout("rcx") _, lateout("r11") _); }
            } else if mode == 16 {
                install(4, 0);
                // Delivery cannot build a frame on this deliberately invalid
                // stack. The kernel must terminate us before attempting IRET.
                unsafe {
                    asm!("mov rsp, 1", "syscall", "ud2", in("rax") 62u64,
                        in("rdi") getpid(), in("rsi") SIGUSR1 as u64, options(noreturn));
                }
            } else {
                install(mode, 0);
                raise(SIGUSR1);
            }
            panic!("bad signal frame must terminate the child");
        }
        let mut status = 0;
        assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
        assert_eq!(status, SIGSEGV);
    }
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 { raise(SIGUSR2); panic!("default action must terminate"); }
    let mut status = 0;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, SIGUSR2);
    println!("[signal-delivery] PASS");
    0
}
