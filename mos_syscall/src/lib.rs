#![no_std]

extern crate alloc;

use crate::fs::{
    raw_sys_access, raw_sys_chmod, raw_sys_close, raw_sys_dup3, raw_sys_faccessat,
    raw_sys_fchmod, raw_sys_fcntl, raw_sys_fstat, raw_sys_ftruncate, raw_sys_link, raw_sys_lseek,
    raw_sys_mkdir, raw_sys_mkfifo, raw_sys_open, raw_sys_pipe, raw_sys_read, raw_sys_readlink,
    raw_sys_readv, raw_sys_rename, raw_sys_rmdir, raw_sys_stat, raw_sys_symlink,
    raw_sys_truncate, raw_sys_umask, raw_sys_unlink, raw_sys_write, raw_sys_writev, sys_chdir,
    sys_getdents,
};
use crate::mem::{sys_madvise, sys_mmap, sys_mprotect, sys_munmap};
use crate::process::{
    sys_brk, sys_exit, sys_fork, sys_getcpu, sys_sleep, sys_wait, sys_yield,
};
use crate::random::sys_rand;
use log::info;
use mos_abi::error::KError;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_abi::syscall::Syscall;
use mos_abi::time::Timespec;
use mos_arch::register_syscall;
use mos_arch::structs::SyscallContext;
use mos_lib::align::{align_up, is_aligned};
use mos_task::process::KernelStack;
use mos_task::thread::{Priority, ThreadControlBlock};
use mos_task::{current_tcb, enqueue_task};
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;

pub mod display;
pub mod fs;
pub mod mem;
pub mod process;
pub mod random;
pub mod task;
pub mod time;

/// Set rax to a Linux-style `-errno` return value.
#[inline]
pub fn ret_err(context: &mut SyscallContext, err: KError) {
    context.scratch.rax = (-err.to_errno()) as u64;
}

/// Same as [`ret_err`], for VFS-layer errors.
#[inline]
pub fn ret_vfs_err(context: &mut SyscallContext, err: vfs_common::VfsError) {
    context.scratch.rax = (-vfs_common::vfs_error_to_errno(err)) as u64;
}

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
        Syscall::Readv => raw_sys_readv(context, args),
        Syscall::Writev => raw_sys_writev(context, args),
        Syscall::Open => raw_sys_open(context, args),
        Syscall::Close => raw_sys_close(context, args),
        Syscall::Mkfifo => raw_sys_mkfifo(context, args),
        Syscall::Pipe => raw_sys_pipe(context, args),
        Syscall::Exit => sys_exit(args[0] as _),
        Syscall::ExitGroup => crate::process::sys_exit_group(args[0] as _),
        Syscall::Yield => sys_yield(),
        Syscall::GetCpu => context.scratch.rax = sys_getcpu(),
        Syscall::GetPid => context.scratch.rax = crate::process::sys_getpid(),
        Syscall::GetPpid => context.scratch.rax = crate::process::sys_getppid(),
        Syscall::Getuid | Syscall::Geteuid | Syscall::Getgid | Syscall::Getegid => {
            context.scratch.rax = 0;
        }
        Syscall::Fork => sys_fork(context),
        Syscall::Wait => context.scratch.rax = sys_wait(args[0]) as _,
        Syscall::Execve => crate::process::handle_execve(context, args),
        Syscall::Rand => context.scratch.rax = sys_rand(),
        Syscall::SetTidAddress => {
            context.scratch.rax = crate::process::sys_set_tid_address(args[0])
        }
        Syscall::Uname => match crate::process::sys_uname(args[0]) {
            Ok(()) => context.scratch.rax = 0,
            Err(e) => ret_err(context, e),
        },
        Syscall::ArchPrctl => match crate::process::sys_arch_prctl(args[0], args[1]) {
            Ok(ret) => context.scratch.rax = ret,
            Err(e) => ret_err(context, e),
        },
        Syscall::Nanosleep => {
            let req = args[0] as *const Timespec;
            if req.is_null() {
                ret_err(context, KError::InvalidArg);
                return;
            }
            let ts = unsafe { *req };
            if ts.tv_sec < 0 || !(0..1_000_000_000).contains(&ts.tv_nsec) {
                ret_err(context, KError::InvalidArg);
                return;
            }
            let ms = ts.tv_sec as u64 * 1000 + ts.tv_nsec as u64 / 1_000_000;
            sys_sleep(ms);
        }
        Syscall::Brk => context.scratch.rax = sys_brk(args[0]),
        Syscall::Mmap => {
            if !is_aligned(args[0], 4096) || !is_aligned(args[5], 4096) {
                ret_err(context, KError::InvalidArg);
                return;
            }

            let start = if args[0] != 0 {
                Some(Page::containing_address(VirtAddr::new(args[0])))
            } else {
                None
            };
            let length = args[1];
            let pages = align_up(length, 4096) / 4096;
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
                    ret_err(context, e);
                }
            }
        }
        Syscall::Munmap => {
            let start = Page::containing_address(VirtAddr::new(args[0]));
            let length = args[1];
            let pages = align_up(length, 4096) / 4096;
            let ret = sys_munmap(start, pages as _);
            match ret {
                Ok(_) => context.scratch.rax = 0,
                Err(e) => {
                    info!("sys_munmap: {:?}", e);
                    ret_err(context, e);
                }
            }
        }
        Syscall::Mprotect => match sys_mprotect(args[0], args[1], args[2]) {
            Ok(_) => context.scratch.rax = 0,
            Err(e) => {
                info!("sys_mprotect: {:?}", e);
                ret_err(context, e);
            }
        },
        Syscall::Madvise => match sys_madvise(args[0], args[1], args[2]) {
            Ok(_) => context.scratch.rax = 0,
            Err(e) => ret_err(context, e),
        },

        Syscall::Lseek => raw_sys_lseek(context, args),
        Syscall::Fstat => raw_sys_fstat(context, args),
        Syscall::Stat | Syscall::Lstat => raw_sys_stat(context, args),
        Syscall::GetDents64 => {
            let fd = FileDesc(args[0] as usize);
            let slice: &mut [u8] =
                unsafe { core::slice::from_raw_parts_mut(args[1] as _, args[2] as _) };
            match sys_getdents(fd, slice) {
                Ok(n) => context.scratch.rax = n as _,
                Err(e) => {
                    info!("sys_getdents: {:?}", e);
                    ret_vfs_err(context, e)
                }
            }
        }
        Syscall::ChDir => {
            let fd = FileDesc(args[0] as usize);
            match sys_chdir(fd) {
                Ok(_) => context.scratch.rax = 0,
                Err(e) => {
                    info!("sys_chdir: {:?}", e);
                    ret_vfs_err(context, e)
                }
            }
        }
        Syscall::Mkdir => raw_sys_mkdir(context, args),
        Syscall::Unlink => raw_sys_unlink(context, args),
        Syscall::Rmdir => raw_sys_rmdir(context, args),
        Syscall::Rename => raw_sys_rename(context, args),
        Syscall::Link => raw_sys_link(context, args),
        Syscall::Symlink => raw_sys_symlink(context, args),
        Syscall::Readlink => raw_sys_readlink(context, args),
        Syscall::Access | Syscall::Faccessat => {
            if syscall_id == Syscall::Faccessat as u64 {
                raw_sys_faccessat(context, args)
            } else {
                raw_sys_access(context, args)
            }
        }
        Syscall::Umask => raw_sys_umask(context, args),
        Syscall::Truncate => raw_sys_truncate(context, args),
        Syscall::Ftruncate => raw_sys_ftruncate(context, args),
        Syscall::Chmod => raw_sys_chmod(context, args),
        Syscall::Fchmod => raw_sys_fchmod(context, args),
        Syscall::Dup => {
            let fd = FileDesc(args[0] as usize);
            let tcb = current_tcb();
            let files = &tcb.process.files();
            match files.lock().dup(fd) {
                None => ret_err(context, KError::BadFd),
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
                ret_err(context, KError::BadFd);
            }
        }
        Syscall::Fcntl => raw_sys_fcntl(context, args),
        Syscall::Dup3 => raw_sys_dup3(context, args),

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

        Syscall::ClockGetTime => crate::time::raw_sys_clock_gettime(context, args),
        Syscall::GetTimeOfDay => crate::time::raw_sys_gettimeofday(context, args),

        Syscall::ClockTicks => {
            context.scratch.rax = mos_platform::timer::monotonic_ticks();
        }
        Syscall::ClockFreq => {
            context.scratch.rax = mos_platform::acpi::PM_TIMER_HZ;
        }

        _ => {
            info!("unknown syscall {}: return -ENOSYS", syscall_id);
            ret_err(context, KError::UnknownSyscall);
        }
    }
}

register_syscall!(fast_syscall, |sys_args| {
    syscall(sys_args);
});
