#![allow(unused)]
use mos_abi::error::{KError, KResult};

use crate::task::load_elf_from_file;
use alloc::borrow::ToOwned;
use alloc::boxed::Box;
use alloc::ffi::CString;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::task::Wake;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use core::mem::ManuallyDrop;
use core::ops::Sub;
use core::ptr::NonNull;
use core::sync::atomic::Ordering;
use core::task::{RawWaker, RawWakerVTable, Waker};
use log::info;
use mos_abi::ffi::NullEndPointerArray;
use mos_abi::io::Read;
use mos_abi::stack::PushStack;
use mos_arch::structs::{
    FpuState, InterruptStack, IretRegisters, PreservedRegisters, CpuContext, ScratchRegisters,
    SyscallContext,
};
use mos_arch::{init_fpu_state, save_fpu_state};
use mos_fs_runtime::open;
use mos_fs_runtime::pipe::make_pipe;
use mos_lib::align::align_down;
use mos_lib::sync::KRef;
use mos_mem::PageTableManager;
use mos_platform::timer::{add_timer, monotonic_deadline_after_ms};
use mos_task::enqueue_task;
use mos_task::process::{KernelStack, ProcessControlBlock};
use mos_task::scheduler::{ParkDecision, exit_current, park_current, wake_blocked_task};
use mos_task::thread::{ThreadControlBlock, ThreadState, ThreadStateValue};
use mos_task::{current_tcb, schedule};
use vfs_common::{OpenFlags, VfsResult};
use x86_64::VirtAddr;
use x86_64::instructions::tlb;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::Page;

pub fn sys_getcpu() -> u64 {
    unsafe { mos_arch::percpu::current_raw().cpu_id as u64 }
}

pub fn sys_yield() {
    current_tcb().set_need_schedule();
}

pub fn sys_fork(frame: &mut SyscallContext) {
    let parent_tcb = current_tcb();
    let parent_pcb = &parent_tcb.process;
    let child_result = parent_pcb.fork();
    // Even a failed fork may already have made some parent pages read-only
    // for CoW. Invalidate remote writable translations on both paths.
    mos_platform::smp::tlb_shootdown();
    let child_pcb = match child_result {
        Ok(child) => child,
        Err(error) => {
            crate::ret_err(frame, error);
            return;
        }
    };

    let mut scratch = frame.scratch.clone();
    scratch.rax = 0;

    frame.scratch.rax = child_pcb.pid.as_u64();

    let frame = InterruptStack {
        iret: IretRegisters {
            rip: frame.iret.rip,
            rsp: frame.iret.rsp,
            cs: child_pcb.context.cs.0 as _,
            ss: child_pcb.context.ss.0 as _,
            rflags: frame.iret.rflags,
        },
        scratch,
        preserved: frame.preserved.clone(),
    };

    let mut fpu_state = FpuState::default();
    save_fpu_state(&mut fpu_state);

    let child_tcb = ThreadControlBlock::create_with_context(
        ThreadState::new(ThreadStateValue::Ready),
        child_pcb,
        frame,
        fpu_state,
        KernelStack::new(mos_task::process::DEFAULT_KERNEL_STACK_PAGES),
        parent_tcb.priority,
    );
    // the child shares the parent's address space (COW), so it must keep
    // the same user FS base (TLS pointer)
    child_tcb.set_fs_base(parent_tcb.fs_base());
    child_tcb.signals.lock().blocked = parent_tcb.signals.lock().blocked;
    enqueue_task(child_tcb);
}

/// 从 syscall dispatcher 接收原始参数，解析后调用 sys_execve
pub fn handle_execve(context: &mut SyscallContext, args: [u64; 6]) {
    let exec_path = unsafe { CStr::from_ptr(args[0] as *const _) };
    let exec_args = unsafe { NullEndPointerArray::from_ptr(args[1] as *const *const c_char) };
    let exec_envp = unsafe { NullEndPointerArray::from_ptr(args[2] as *const *const c_char) };
    info!("[syscall] sys_execve");
    let Ok(path) = exec_path.to_str() else {
        info!("path is not valid UTF-8");
        crate::ret_err(context, KError::InvalidArg);
        return;
    };
    let path = path.to_owned();
    let argv: Vec<CString> = exec_args.to_vec_cstring();
    let envp: Vec<CString> = exec_envp.to_vec_cstring();
    match sys_execve(context, path, argv, envp) {
        Ok(_) => context.scratch.rax = 0,
        Err(e) => {
            // execve must return -errno to the caller, not kill the process
            info!("sys_execve: {:?}", e);
            crate::ret_err(context, e);
        }
    }
}

pub fn sys_execve(
    frame: &mut SyscallContext,
    path: String,
    argv: Vec<CString>,
    envp: Vec<CString>,
) -> KResult<()> {
    info!("{:?}{:?}{:?}", path, argv, envp);

    let mut f = open(&path, OpenFlags::READ).map_err(|e| e.as_str())?;
    let current_tcb = current_tcb();
    let current_pcb = &current_tcb.process;

    /// unsafe
    current_pcb.image_resources.lock().clear();
    current_pcb.mem.clear();

    let load_info = load_elf_from_file(current_pcb, &f)?;

    // exec replaces the old image, including its thread-exit address. Keeping
    // musl's old clear_child_tid would write into an unmapped address on exit.
    current_tcb.set_clear_child_tid(0);
    current_pcb.signals.lock().on_exec();

    let stack = current_pcb.prepare_user_stack()?;
    mos_task::job::on_exec(&current_pcb.shared);
    frame.preserved = PreservedRegisters::default();
    frame.scratch = ScratchRegisters::default();
    frame.iret.rip = load_info.entry.as_u64();
    frame.iret.rflags = CpuContext::DEFAULT_RFLAGS.bits() | 2;
    current_tcb.init_fpu_state();
    tlb::flush_all();

    unsafe fn copy_str_array(ptr_top: &mut *mut u8, data: &[CString]) -> Vec<*mut u8> {
        unsafe {
            data.iter()
                .rev()
                .map(|s| {
                    let data = s.as_bytes_with_nul();
                    let start = ptr_top.sub(data.len());
                    core::ptr::copy_nonoverlapping(data.as_ptr(), start, data.len());
                    *ptr_top = start;
                    start
                })
                .collect()
        }
    }

    unsafe {
        let mut stack_top: *mut u8 = stack.start_address().as_mut_ptr();

        // write strings at the top of the stack
        let envp_pos = copy_str_array(&mut stack_top, &envp);
        let argv_pos = copy_str_array(&mut stack_top, &argv);

        // AT_RANDOM: 16 random bytes at a 16-byte aligned address below the strings
        stack_top = align_down(stack_top as u64, 16) as *mut u8;
        let random_addr = stack_top.sub(16);
        let rand0 = crate::random::sys_rand();
        let rand1 = crate::random::sys_rand();
        (random_addr as *mut u64).write(rand0);
        (random_addr as *mut u64).add(1).write(rand1);

        // auxv: (key, value) pairs, NULL-terminated, Linux layout
        let auxv: [(u64, u64); 13] = [
            (AT_PHDR, load_info.phdr),
            (AT_PHENT, load_info.phent as u64),
            (AT_PHNUM, load_info.phnum as u64),
            (AT_PAGESZ, 4096),
            (AT_ENTRY, load_info.entry.as_u64()),
            (AT_RANDOM, random_addr as u64),
            (AT_UID, 0),
            (AT_EUID, 0),
            (AT_GID, 0),
            (AT_EGID, 0),
            (AT_CLKTCK, 1000), // OS_HZ
            (AT_SECURE, 0),
            (AT_NULL, 0),
        ];
        let mut stack_top = random_addr.cast::<u64>();
        for &(key, value) in auxv.iter().rev() {
            stack_top = stack_top.sub(1);
            stack_top.write(value);
            stack_top = stack_top.sub(1);
            stack_top.write(key);
        }

        // NULL terminating envp (highest of the pointer block)
        stack_top = stack_top.sub(1);
        stack_top.write(0);

        // envp pointers
        for ptr in envp_pos {
            stack_top = stack_top.sub(1);
            stack_top.write(ptr as _);
        }

        // NULL terminating argv
        stack_top = stack_top.sub(1);
        stack_top.write(0);

        // argv pointers
        for ptr in argv_pos {
            stack_top = stack_top.sub(1);
            stack_top.write(ptr as _);
        }

        // argc — rsp points here at entry (Linux convention)
        stack_top = stack_top.sub(1);
        stack_top.write(argv.len() as _);

        frame.iret.rsp = stack_top as _;
    }

    Ok(())
}

pub fn sys_spawn() -> isize {
    0
}

pub fn sys_sleep(ms: u64) {
    let deadline = monotonic_deadline_after_ms(ms);
    park_current(|task| {
        if add_timer(deadline, Some(Waker::from(task.clone()))) {
            ParkDecision::Park(())
        } else {
            ParkDecision::Unpark
        }
    });
}

/// Linux wait4 minimal exit-only subset. A child record stays in `childs`
/// until output has been copied successfully; failed copies can be retried.
pub fn sys_wait4(pid: i32, status: u64, options: u32, rusage: u64) -> KResult<u64> {
    use mos_abi::process::{WNOHANG, WUNTRACED, exit_status};
    // Stopped states are not generated yet, but accepting WUNTRACED is
    // essential for an interactive shell's ordinary exit wait. Until stop/
    // continue lands, it simply broadens no result set.
    if options & !(WNOHANG | WUNTRACED) != 0 {
        return Err(KError::InvalidArg);
    }
    // Resource accounting remains unimplemented.
    if rusage != 0 {
        return Err(KError::Unsupported);
    }
    if pid == i32::MIN { return Err(KError::NoProcess); }
    let current = current_tcb();
    let pcb = &current.process;
    // pid=0 means the caller's group at syscall entry, not at each wakeup.
    let group = if pid == 0 { mos_task::job::group_of(&pcb.shared) }
        else if pid < -1 { -(pid as i64) as u64 } else { 0 };
    loop {
        // Hold the notification lock from inspection through registration.
        // Exit takes the same lock: it cannot slip between the scan and park.
        let mut queue = pcb.child_exit_queue.lock();
        let mut children = pcb.childs.lock();
        let matches = |child: &Arc<mos_task::process::SharedState>| {
            if pid == -1 { true }
            else if pid > 0 { child.child_pid.as_u64() == pid as u64 }
            else { mos_task::job::group_of(child) == group }
        };
        if !children.iter().any(matches) {
            return Err(KError::NoChild);
        }
        if let Some(index) = children.iter().position(|child| {
            matches(child) && child.exited.load(Ordering::Acquire)
        }) {
            drop(queue);
            let child = &children[index];
            let result = child.child_pid.as_u64();
            if status != 0 {
                let encoded = child.wait_status.load(Ordering::Relaxed);
                let copied = pcb.mem.copy_to_user(status, &encoded.to_ne_bytes());
                // The copy can resolve COW even if a later page fails.
                mos_platform::smp::tlb_shootdown();
                copied?;
            }
            // The children lock serializes competing waiters: only one can reap.
            children.swap_remove(index);
            return Ok(result);
        }
        drop(children);
          if options & WNOHANG != 0 {
              return Ok(0);
          }
          if crate::signal::interrupts_wait(&current) {
              return Err(KError::Interrupted);
          }
        park_current(|task| {
            queue.wait_task(task.clone());
            ParkDecision::Park(queue)
        });
        // Any child may have exited; rescan rather than assuming this wakeup
        // belongs to the requested PID (or that another waiter has not reaped it).
    }
}
pub fn sys_exit(exit_code: i32) -> ! {
    exit_with_status(mos_abi::process::exit_status(exit_code))
}

/// Wait status low bits distinguish signal death from a normal exit code.
pub fn sys_exit_signal(signal: i32) -> ! {
    exit_with_status(signal & 0x7f)
}

fn exit_with_status(status: i32) -> ! {
    let current = current_tcb();
    current.process.image_resources.lock().clear();
    clear_child_tid_if_any(&current);
    crate::signal::notify_child_exit(&current, status);
    drop(current);
    exit_current();
}

/// exit_group(2): terminate all threads of the calling process.
///
/// TODO(phase 3): kill sibling threads. Until then every user program is
/// effectively single-threaded, so it behaves like sys_exit.
pub fn sys_exit_group(exit_code: i32) -> ! {
    sys_exit(exit_code)
}

fn clear_child_tid_if_any(tcb: &mos_task::thread::ThreadControlBlock) {
    let addr = tcb.clear_child_tid();
    if addr != 0 {
        // TODO(phase 3): futex-wake the waiters on this address
        // Like Linux, an inaccessible exit notification address must not turn
        // process exit into a kernel fault. Futex wake remains a separate task.
        let _ = tcb.process.mem.copy_to_user(addr, &0u32.to_ne_bytes());
        mos_platform::smp::tlb_shootdown();
    }
}

pub fn sys_set_tid_address(tidptr: u64) -> u64 {
    let current = current_tcb();
    current.set_clear_child_tid(tidptr);
    current.tid.as_u64()
}

pub fn sys_getpid() -> u64 {
    current_tcb().process.pid.as_u64()
}

pub fn sys_getppid() -> u64 {
    current_tcb().process.ppid.load(Ordering::Acquire)
}

/// uname(2) — fill a Linux-layout struct utsname in user memory.
pub fn sys_uname(ptr: u64) -> KResult<()> {
    use mos_abi::time::Utsname;
    if ptr == 0 {
        return Err(KError::InvalidArg);
    }
    unsafe {
        core::ptr::write(ptr as *mut Utsname, Utsname::default());
    }
    Ok(())
}

// arch_prctl(2) request codes
const ARCH_SET_GS: u64 = 0x1001;
const ARCH_SET_FS: u64 = 0x1002;
const ARCH_GET_FS: u64 = 0x1003;
const ARCH_GET_GS: u64 = 0x1004;

// Linux auxv entry types
const AT_NULL: u64 = 0;
const AT_PHDR: u64 = 3;
const AT_PHENT: u64 = 4;
const AT_PHNUM: u64 = 5;
const AT_PAGESZ: u64 = 6;
const AT_ENTRY: u64 = 9;
const AT_UID: u64 = 11;
const AT_EUID: u64 = 12;
const AT_GID: u64 = 13;
const AT_EGID: u64 = 14;
const AT_CLKTCK: u64 = 17;
const AT_SECURE: u64 = 23;
const AT_RANDOM: u64 = 25;

/// arch_prctl(2): thread-pointer (TLS) control. musl calls ARCH_SET_FS during
/// startup, before touching any TLS variable.
pub fn sys_arch_prctl(code: u64, addr: u64) -> KResult<u64> {
    use x86_64::registers::model_specific::{FsBase, Msr};
    match code {
        ARCH_SET_FS => {
            if addr >= 0x0000_8000_0000_0000 { return Err(KError::InvalidArg); }
            // Updating hardware and the saved context is one operation. A
            // preemption between the two would restore the stale saved base.
            x86_64::instructions::interrupts::without_interrupts(|| {
                current_tcb().set_fs_base(addr);
                FsBase::write(VirtAddr::new(addr));
            });
            Ok(0)
        }
        ARCH_GET_FS => Ok(FsBase::read().as_u64()),
        ARCH_SET_GS => Err(KError::InvalidArg), // user GS base not supported yet
        ARCH_GET_GS => Ok(0),
        _ => Err(KError::InvalidArg),
    }
}

pub fn sys_brk(brk: u64) -> u64 {
    let current = current_tcb();
    current.process.mem.addr_space.access(|mm| {
        if brk != 0 {
            // Linux returns the new brk on success, -ENOMEM on failure
            mm.set_brk(Page::containing_address(VirtAddr::new(brk)))
                .map(|_| brk)
                .unwrap_or((-KError::OutOfMem.to_errno()) as u64)
        } else {
            mm.current_brk().start_address().as_u64() as _
        }
    })
}
