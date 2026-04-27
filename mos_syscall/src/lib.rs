#![no_std]

extern crate alloc;

use crate::fs::{
    raw_sys_close, raw_sys_mkfifo, raw_sys_open, raw_sys_pipe, raw_sys_read, raw_sys_write,
    sys_chdir, sys_getdents,
};
use crate::mem::{sys_mmap, sys_munmap};
use crate::process::{sys_brk, sys_execve, sys_exit, sys_fork, sys_sleep, sys_wait, sys_yield};
use crate::random::sys_rand;
use alloc::borrow::ToOwned;
use alloc::ffi::CString;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use core::mem::needs_drop;
use log::info;
use mos_abi::ffi::NullEndPointerArray;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_abi::syscall::Syscall;
use mos_arch::register_syscall;
use mos_arch::structs::SyscallContext;
use mos_lib::align::{align_up, is_aligned};
use mos_task::current_tcb;
use vfs_common::UserDirEntry;
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use mos_display::{new_window, Rect};

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
        Syscall::Fork => sys_fork(context),
        Syscall::Wait => context.scratch.rax = sys_wait(args[0]) as _,
        Syscall::Execve => {
            let exec_path = unsafe {
                let ptr = args[0] as *const _;
                CStr::from_ptr(ptr)
            };
            let exec_args =
                unsafe { NullEndPointerArray::from_ptr(args[1] as *const *const c_char) };
            let exec_envp =
                unsafe { NullEndPointerArray::from_ptr(args[2] as *const *const c_char) };
            info!("sys_execve");
            info!("{:X?}", args);
            info!("{:?}{:?}{:?}", exec_path, exec_args, exec_envp);
            let Ok(path) = exec_path.to_str() else {
                info!("path is not valid UTF-8");
                context.scratch.rax = u64::MAX;
                return;
            };
            let path = path.to_owned();
            let argv: Vec<CString> = exec_args.to_vec_cstring();
            let envp: Vec<CString> = exec_envp.to_vec_cstring();
            match sys_execve(context, path, argv, envp) {
                Ok(_) => context.scratch.rax = 0,
                Err(e) => {
                    info!("sys_execve: {:?}", e);
                    // TODO
                    // if the error is file open error, it could be a recoverable error.
                    // unrecoverable error must exit to prevent sysretq to userspace
                    context.scratch.rax = u64::MAX;
                    sys_exit(-1);
                }
            }
        }
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
            let files = &tcb.process.files;
            match files.lock().dup(fd) {
                None => context.scratch.rax = u64::MAX,
                Some(fd) => context.scratch.rax = fd.0 as _,
            }
        }
        Syscall::DupTo => {
            let src = FileDesc(args[0] as usize);
            let dst = FileDesc(args[1] as usize);
            let tcb = current_tcb();
            let files = &tcb.process.files;
            if files.lock().dup_to(src, dst) {
                context.scratch.rax = 0;
            } else {
                context.scratch.rax = u64::MAX;
            }
        }
        Syscall::CreateWindow => {
            let x = args[0] as usize;
            let y = args[1] as usize;
            let w = args[2] as usize;
            let h = args[3] as usize;
            let rect = Rect { x, y, w, h };

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
