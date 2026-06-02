#![no_std]

extern crate alloc;

use crate::fs::{
    raw_sys_close, raw_sys_mkfifo, raw_sys_open, raw_sys_pipe, raw_sys_read, raw_sys_write,
    sys_chdir, sys_getdents,
};
use crate::mem::{sys_mmap, sys_munmap};
use crate::process::{
    sys_brk, sys_execve, sys_exit, sys_fork, sys_getcpu, sys_sleep, sys_wait, sys_yield,
};
use crate::random::sys_rand;
use alloc::borrow::ToOwned;
use alloc::ffi::CString;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use log::info;
use mos_abi::ffi::NullEndPointerArray;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_abi::syscall::Syscall;
use mos_arch::register_syscall;
use mos_arch::structs::SyscallContext;
use mos_display::Rect;
use mos_display::window::{SharedSurface, Window};
use mos_lib::align::{align_up, is_aligned};
use mos_mem::{PhysicalFrameManager, phys_to_virt};
use mos_mem::virt::{MemoryArea, MemoryFlags};
use mos_task::process::{KernelStack, ProcessControlBlock};
use mos_task::thread::{Priority, ThreadControlBlock};
use mos_task::{current_tcb, enqueue_task};
use spin::Mutex;
use vfs_common::UserDirEntry;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};

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
        Syscall::Execve => {
            let exec_path = unsafe {
                let ptr = args[0] as *const _;
                CStr::from_ptr(ptr)
            };
            let exec_args =
                unsafe { NullEndPointerArray::from_ptr(args[1] as *const *const c_char) };
            let exec_envp =
                unsafe { NullEndPointerArray::from_ptr(args[2] as *const *const c_char) };
            info!("[syscall] sys_execve");
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

        Syscall::CreateWindow => {
            let x = args[0] as usize;
            let y = args[1] as usize;
            let w = args[2] as usize;
            let h = args[3] as usize;
            let out_ptr = args[4] as *mut u64;

            if w == 0 || h == 0 || out_ptr.is_null() {
                context.scratch.rax = u64::MAX;
                return;
            }

            // 计算物理页数
            let pixel_bytes = w * h * core::mem::size_of::<mos_display::RGBA>();
            let npages = (align_up(pixel_bytes as u64, 4096) / 4096) as usize;

            // 分配连续物理帧
            let Some(frames) = PhysicalFrameManager.global_allocate_frames(npages) else {
                context.scratch.rax = u64::MAX;
                return;
            };
            let phys_start = frames.start.start_address();
            let phys_end = frames.end.start_address();

            // 内核 VA (phys_to_virt 固定偏移)
            let kernel_va = phys_to_virt(phys_start);
            let ptr = kernel_va.as_mut_ptr::<mos_display::RGBA>();

            // 在用户虚拟地址空间分配范围
            let tcb = current_tcb();
            let pid = tcb.process.pid;
            let Some(user_range) = tcb.process.mem.find_free_area(npages as u64) else {
                context.scratch.rax = u64::MAX;
                return;
            };
            let user_va = user_range.start.start_address().as_u64();

            // 注册 VMA
            let mem_flags = MemoryFlags::new(
                MemoryProtFlags::READ_WRITE,
                MemoryMapFlags::PRIVATE,
            );
            if tcb.process.mem.addr_space.access(|mm| {
                mm.try_add_area(MemoryArea::new_anon(user_range, mem_flags))
            }).is_err() {
                context.scratch.rax = u64::MAX;
                return;
            }

            // 热切映射所有物理帧到用户页表
            let pte_flags = PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE
                | PageTableFlags::USER_ACCESSIBLE;
            let map_result = tcb.process.mem.addr_space.access(|mm| {
                for i in 0..npages {
                    let page = Page::<Size4KiB>::containing_address(
                        VirtAddr::new(user_va + (i * 4096) as u64),
                    );
                    let frame = PhysFrame::<Size4KiB>::containing_address(
                        PhysAddr::new(phys_start.as_u64() + (i * 4096) as u64),
                    );
                    mm.map_page_eagerly(page, frame, pte_flags)
                        .map_err(|_| "map failed")?;
                }
                Ok::<_, &'static str>(())
            });
            if map_result.is_err() {
                context.scratch.rax = u64::MAX;
                return;
            }

            // 清零像素缓冲区
            unsafe {
                core::ptr::write_bytes(kernel_va.as_mut_ptr::<u8>(), 0, pixel_bytes);
            }

            // 创建 SharedSurface + Window + 注册到 compositor
            let shared = unsafe { SharedSurface::new(w, h, w, ptr) };
            let window = Arc::new(Mutex::new(Window::new(x, y, shared)));
            let window_id = mos_display::register_user_window(window, pid.as_u64());

            // 向用户输出缓冲区写入结果
            unsafe {
                out_ptr.add(0).write_volatile(window_id);
                out_ptr.add(1).write_volatile(user_va);
                out_ptr.add(2).write_volatile(w as u64);
                out_ptr.add(3).write_volatile(h as u64);
                out_ptr.add(4).write_volatile(w as u64); // pitch == width
            }
            context.scratch.rax = 0;
        }

        Syscall::WindowMarkDirty => {
            let window_id = args[0];
            let x = args[1] as usize;
            let y = args[2] as usize;
            let w = args[3] as usize;
            let h = args[4] as usize;
            mos_display::mark_user_window_dirty(window_id, x, y, w, h);
        }

        Syscall::WindowSync => {
            mos_display::render_to_framebuffer();
        }

        Syscall::DestroyWindow => {
            let window_id = args[0];
            mos_display::destroy_user_window(window_id);
        }

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
