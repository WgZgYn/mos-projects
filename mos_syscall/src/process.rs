#![allow(unused)]

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
    FpuState, InterruptStack, IretRegisters, PreservedRegisters, ProcessContext, ScratchRegisters,
    SyscallContext,
};
use mos_arch::{init_fpu_state, save_fpu_state};
use mos_fs_runtime::open;
use mos_fs_runtime::pipe::make_pipe;
use mos_lib::align::align_down;
use mos_lib::sync::KRef;
use mos_mem::PageTableManager;
use mos_platform::OS_HZ;
use mos_platform::timer::{add_timer, get_tick};
use crate::task::load_elf_from_file;
use mos_task::enqueue_task;
use mos_task::process::{KernelStack, ProcessControlBlock};
use mos_task::thread::{ThreadControlBlock, ThreadState, ThreadStateValue};
use mos_task::{SCHEDULER, current_tcb, schedule};
use vfs_common::{OpenFlags, VfsResult};
use x86_64::VirtAddr;
use x86_64::instructions::tlb;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::structures::paging::Page;

pub fn sys_yield() {
    current_tcb().set_need_schedule();
}

pub fn sys_fork(frame: &mut SyscallContext) {
    let parent_tcb = current_tcb();
    let parent_pcb = &parent_tcb.process;
    let Ok(child_pcb) = parent_pcb.fork() else {
        frame.scratch.rax = u64::MAX;
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
    enqueue_task(child_tcb);
}

pub fn sys_execve(
    frame: &mut SyscallContext,
    path: String,
    argv: Vec<CString>,
    envp: Vec<CString>,
) -> Result<(), &'static str> {
    info!("{:?}{:?}{:?}", path, argv, envp);

    let mut f = open(&path, OpenFlags::READ).map_err(|e| e.as_str())?;
    let current_tcb = current_tcb();
    let current_pcb = &current_tcb.process;

    /// unsafe
    current_pcb.mem.clear();

    let entry = load_elf_from_file(current_pcb, &f)?;

    let stack = current_pcb.prepare_user_stack()?;
    frame.preserved = PreservedRegisters::default();
    frame.scratch = ScratchRegisters::default();
    frame.scratch.rcx = entry.as_u64();
    frame.scratch.r11 = ProcessContext::DEFAULT_RFLAGS.bits();
    unsafe {
        let fpu = &mut (*current_tcb.context.get()).fpu;
        init_fpu_state(fpu);
    }
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

        // write str
        let envp_pos = copy_str_array(&mut stack_top, &envp);
        let argv_pos = copy_str_array(&mut stack_top, &argv);

        // align
        stack_top = align_down(stack_top as _, 8) as _;

        // write NULL between strings and envp
        let mut stack_top = stack_top.cast::<u64>().sub(1);
        stack_top.write(0);

        // write envp
        for ptr in envp_pos {
            stack_top = stack_top.sub(1);
            stack_top.write(ptr as _);
        }

        // write NULL between envp and argv
        stack_top = stack_top.sub(1);
        stack_top.write(0);

        // write argv
        for ptr in argv_pos {
            stack_top = stack_top.sub(1);
            stack_top.write(ptr as _);
        }

        // write argc
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
    let current_tick = get_tick();
    let target_tick = current_tick + ms * 1000 / OS_HZ;
    let current = current_tcb();
    current.set_state(ThreadStateValue::Blocked);
    add_timer(target_tick, Some(Waker::from(current)));
    schedule();
}

/// ### child process must set the exited `true` when exiting to avoid parent process waiting infinitely
pub fn sys_wait(pid: u64) -> i32 {
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
        return -1;
    };

    loop {
        if child.exited.load(Ordering::Acquire) {
            break;
        }
        child.wait_queue.wait(current_tcb.clone());
        schedule();
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
        return child.exit_code.load(Ordering::Acquire);
    }
    -1
}

pub fn sys_exit(exit_code: i32) -> ! {
    let current = current_tcb();
    let shared = &current.process.shared;
    shared.exited.store(true, Ordering::Release);
    shared.exit_code.store(exit_code, Ordering::Release);
    shared.wait_queue.wake_all();
    current.set_state(ThreadStateValue::Zombied);
    drop(current);
    schedule();
    unreachable!("return from exit thread");
}

pub fn sys_brk(brk: u64) -> u64 {
    let current = current_tcb();
    current.process.mem.addr_space.access(|mm| {
        if brk != 0 {
            mm.set_brk(Page::containing_address(VirtAddr::new(brk)))
                .map(|_| 0)
                .unwrap_or(u64::MAX)
        } else {
            mm.current_brk().start_address().as_u64() as _
        }
    })
}
