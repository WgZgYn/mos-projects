//! Raw Linux-kernel signal-set wrappers (64 bits), returning negative errno.
pub use mos_abi::signal::*;

pub fn sigprocmask(how: i32, set: Option<&u64>, old: Option<&mut u64>) -> i64 {
    crate::syscall::sys_rt_sigprocmask(how,
        set.map_or(0, |value| value as *const u64 as u64),
        old.map_or(0, |value| value as *mut u64 as u64))
}

/// Custom handlers need SA_RESTORER and a valid executable restorer address.
pub fn sigaction(signal: i32, action: Option<&KernelSigAction>, old: Option<&mut KernelSigAction>) -> i64 {
    crate::syscall::sys_rt_sigaction(signal,
        action.map_or(0, |value| value as *const KernelSigAction as u64),
        old.map_or(0, |value| value as *mut KernelSigAction as u64))
}

pub fn sigpending(set: &mut u64) -> i64 {
    crate::syscall::sys_rt_sigpending(set as *mut u64 as u64)
}

core::arch::global_asm!(
    ".global __mos_rt_sigreturn",
    "__mos_rt_sigreturn:",
    "mov rax, 15",
    "syscall",
    "ud2",
);
unsafe extern "C" { fn __mos_rt_sigreturn(); }

pub fn restorer_address() -> u64 { __mos_rt_sigreturn as *const () as u64 }
pub fn kill(pid: i32, signal: i32) -> i64 { crate::syscall::sys_kill(pid, signal, false) }
pub fn tkill(tid: i32, signal: i32) -> i64 { crate::syscall::sys_kill(tid, signal, true) }
pub fn gettid() -> i32 { crate::syscall::sys_gettid() as i32 }
pub fn raise(signal: i32) -> i64 {
    tkill(gettid(), signal)
}
