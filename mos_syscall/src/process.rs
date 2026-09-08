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
    let Ok(child_pcb) = parent_pcb.fork() else {
        crate::ret_err(frame, KError::OutOfMem);
        return;
    };

    let mut scratch = frame.scratch.clone();
    scratch.rax = 0;

    frame.scratch.rax = child_pcb.pid.as_u64();

    let frame = InterruptStack {
        iret: IretRegisters {
            rip: frame.scratch.rcx,
            rsp: frame.user_rsp,
            cs: child_pcb.context.cs.0 as _,
            ss: child_pcb.context.ss.0 as _,
            rflags: frame.scratch.r11,
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
    current_pcb.mem.clear();

    let load_info = load_elf_from_file(current_pcb, &f)?;

    let stack = current_pcb.prepare_user_stack()?;
    frame.preserved = PreservedRegisters::default();
    frame.scratch = ScratchRegisters::default();
    frame.scratch.rcx = load_info.entry.as_u64();
    frame.scratch.r11 = CpuContext::DEFAULT_RFLAGS.bits();
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

        frame.user_rsp = stack_top as _;
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

/// ### child process must set the exited `true` when exiting to avoid parent process waiting infinitely
pub fn sys_wait(pid: u64) -> i64 {
    let current_tcb = current_tcb();
    let current_pcb = current_tcb.process.clone();
    let target_pcb_state = current_pcb
        .childs
        .lock()
        .iter()
        .enumerate()
        .find(|(_, s)| s.child_pid.as_u64() == pid)
        .map(|(i, s)| (i, s.clone()));

    let Some((index, child)) = target_pcb_state else {
        return -KError::NoChild.to_errno();
    };

    loop {
        let mut ok = false;
        let mut queue = child.wait_queue.lock();
        park_current(|task| {
            if child.exited.load(Ordering::Acquire) {
                ok = true;
                ParkDecision::Unpark
            } else {
                queue.wait_task(task.clone());
                ParkDecision::Park(queue)
            }
        });
        if ok {
            break;
        }
    }
    drop(child);

    // for Vec struct it needs the position is fixed at index,

    // 1. simply swap_remove(index) but it need the vec is position-stable
    // 2. refind then swap_remove
    // 3. use `Map` struct to make it unique with pid, when new insert will replace the old one,
    // then we get the return code from the prev child Arc ref

    // current_pcb
    //     .childs
    //     .lock()
    //     .swap_remove(index)
    //     .exit_code
    //     .load(Ordering::Acquire)

    let mut childs = current_pcb.childs.lock();
    if let Some(index) = childs
        .iter()
        .enumerate()
        .find(|(_, s)| s.child_pid.as_u64() == pid)
        .map(|(i, _)| i)
    {
        let child = childs.swap_remove(index);
        return child.exit_code.load(Ordering::Acquire) as i64;
    }
    -KError::NoChild.to_errno()
}

pub fn sys_exit(exit_code: i32) -> ! {
    let current = current_tcb();
    clear_child_tid_if_any(&current);
    let shared = &current.process.shared();
    shared.exited.store(true, Ordering::Release);
    shared.exit_code.store(exit_code, Ordering::Release);
    wake_blocked_task(shared.wait_queue.lock().wake_all());
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
        unsafe { (addr as *mut u32).write_volatile(0) };
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
            let base = VirtAddr::new(addr);
            FsBase::write(base);
            current_tcb().set_fs_base(addr);
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
            mm.set_brk(Page::containing_address(VirtAddr::new(brk)))
                .map(|_| 0)
                .unwrap_or((-KError::OutOfMem.to_errno()) as u64)
        } else {
            mm.current_brk().start_address().as_u64() as _
        }
    })
}
