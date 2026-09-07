#![no_std]

extern crate alloc;

use crate::fs::{
    raw_sys_close, raw_sys_mkfifo, raw_sys_open, raw_sys_pipe, raw_sys_read, raw_sys_write,
    sys_chdir, sys_getdents,
};
use crate::mem::{sys_mmap, sys_munmap};
use crate::process::{
    sys_brk, sys_exit, sys_fork, sys_getcpu, sys_sleep, sys_wait, sys_yield,
};
use crate::random::sys_rand;
use log::info;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_abi::syscall::Syscall;
use mos_arch::register_syscall;
use mos_arch::structs::SyscallContext;
use mos_lib::align::{align_up, is_aligned};
use mos_task::process::KernelStack;
use mos_task::thread::{Priority, ThreadControlBlock};
use mos_task::{current_tcb, enqueue_task};
use vfs_common::UserDirEntry;
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;

pub mod display;
pub mod fs;
pub mod mem;
pub mod process;
pub mod random;
pub mod task;

/// syscall_no: rax
/// args: rdi, rsi, rdx
/// return: rax
/// rip: rcx
/// rflags: r11
#[inline(never)]
pub fn syscall(context: &mut SyscallContext) {
    let syscall_id = context.scratch.rax;
    let args = [
        context.scratch.rdi,
        context.scratch.rsi,
        context.scratch.rdx,
        context.scratch.r10,
        context.scratch.r8,
        context.scratch.r9,
    ];

    let syscall = Syscall::from(syscall_id);
    context.scratch.rax = 0;
    match syscall {
        Syscall::Read => raw_sys_read(context, args),
        Syscall::Write => raw_sys_write(context, args),
        Syscall::Open => raw_sys_open(context, args),
        Syscall::Close => raw_sys_close(context, args),
        Syscall::Mkfifo => raw_sys_mkfifo(context, args),
        Syscall::Pipe => raw_sys_pipe(context, args),
        Syscall::Exit => sys_exit(args[0] as _),
        Syscall::Yield => sys_yield(),
        Syscall::GetCpu => context.scratch.rax = sys_getcpu(),
        Syscall::Fork => sys_fork(context),
        Syscall::Wait => context.scratch.rax = sys_wait(args[0]) as _,
        Syscall::Execve => crate::process::handle_execve(context, args),
        Syscall::Rand => context.scratch.rax = sys_rand(),
        Syscall::Sleep => sys_sleep(args[0] as _),
        Syscall::Brk => context.scratch.rax = sys_brk(args[0]),
        Syscall::Mmap => {
            if !is_aligned(args[0], 4096) || !is_aligned(args[5], 4096) {
                context.scratch.rax = u64::MAX;
                return;
            }

            let start = if args[0] != 0 {
                Some(Page::containing_address(VirtAddr::new(args[0])))
            } else {
                None
            };
            let length = args[1];
            let pages = align_up(length, 4096);
            let prot = MemoryProtFlags::from_bits_truncate(args[2] as u32);
            let mapf = MemoryMapFlags::from_bits_truncate(args[3] as _);
            let fd = args[4];
            let offset = args[5];

            let ret = sys_mmap(start, pages, prot, mapf, fd as _, offset);
            match ret {
                Ok(range) => {
                    context.scratch.rax = range.start.start_address().as_u64();
                }
                Err(e) => {
                    info!("sys_mmap: {:?}", e);
                    context.scratch.rax = u64::MAX;
                }
            }
        }
        Syscall::Munmap => {
            let start = Page::containing_address(VirtAddr::new(args[0]));
            let length = args[1];
            let pages = align_up(length, 4096);
            let ret = sys_munmap(start, pages as _);
            match ret {
                Ok(_) => context.scratch.rax = 0,
                Err(e) => {
                    info!("sys_munmap: {:?}", e);
                    context.scratch.rax = u64::MAX;
                }
            }
        }

        Syscall::GetDents => {
            let fd = FileDesc(args[0] as usize);
            let slice: &mut [UserDirEntry] =
                unsafe { core::slice::from_raw_parts_mut(args[1] as _, args[2] as _) };
            match sys_getdents(fd, slice) {
                Ok(n) => context.scratch.rax = n as _,
                Err(e) => {
                    info!("sys_getdents: {:?}", e);
                    context.scratch.rax = u64::MAX
                }
            }
        }
        Syscall::ChDir => {
            let fd = FileDesc(args[0] as usize);
            match sys_chdir(fd) {
                Ok(_) => context.scratch.rax = 0,
                Err(e) => {
                    info!("sys_chdir: {:?}", e);
                    context.scratch.rax = u64::MAX
                }
            }
        }
        Syscall::Dup => {
            let fd = FileDesc(args[0] as usize);
            let tcb = current_tcb();
            let files = &tcb.process.files();
            match files.lock().dup(fd) {
                None => context.scratch.rax = u64::MAX,
                Some(fd) => context.scratch.rax = fd.0 as _,
            }
        }
        Syscall::DupTo => {
            let src = FileDesc(args[0] as usize);
            let dst = FileDesc(args[1] as usize);
            let tcb = current_tcb();
            let files = &tcb.process.files();
            if files.lock().dup_to(src, dst) {
                context.scratch.rax = 0;
            } else {
                context.scratch.rax = u64::MAX;
            }
        }

        Syscall::CreateSemaphore => {
            let init = args[0];
            let index = mos_task::semaphore::SemaphoreTable::create(init as _);
            context.scratch.rax = index as _;
        }
        Syscall::SignalSemaphore => {
            let index = args[0];
            mos_task::semaphore::SemaphoreTable::signal(index as _);
        }
        Syscall::WaitSemaphore => {
            let index = args[0];
            mos_task::semaphore::SemaphoreTable::wait(index as _);
        }

        Syscall::CreateThread => {
            info!("[SYS] create thread");
            let entry = args[0];
            let stack = args[1];

            let pcb = current_tcb().process.clone();
            let tcb = ThreadControlBlock::create(
                pcb,
                VirtAddr::new(entry),
                Some(VirtAddr::new(stack)),
                KernelStack::default(),
                Priority::default(),
            );
            let tid = tcb.tid;
            enqueue_task(tcb);

            context.scratch.rax = tid.as_u64();
        }

        Syscall::CreateWindow => crate::display::sys_create_window(context, args),
        Syscall::WindowMarkDirty => crate::display::sys_window_mark_dirty(args),
        Syscall::WindowSync => crate::display::sys_window_sync(),
        Syscall::DestroyWindow => crate::display::sys_destroy_window(args),

        Syscall::ClockTicks => {
            context.scratch.rax = mos_platform::timer::monotonic_ticks();
        }
        Syscall::ClockFreq => {
            context.scratch.rax = mos_platform::acpi::PM_TIMER_HZ;
        }

        _ => {
            info!("unknown syscall {:?} regard as sys_exit", syscall_id);
            sys_exit(-1)
        }
    }
}

register_syscall!(fast_syscall, |sys_args| {
    syscall(sys_args);
});
