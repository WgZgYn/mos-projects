//! 显示相关系统调用 (CreateWindow, WindowMarkDirty, WindowSync, DestroyWindow)

use alloc::sync::Arc;
use mos_abi::error::KError;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_arch::structs::SyscallContext;
use mos_display::window::{SharedSurface, Window};
use mos_lib::align::align_up;
use mos_mem::virt::{MemoryArea, MemoryFlags};
use mos_mem::{PhysicalFrameManager, phys_to_virt};
use mos_task::current_tcb;
use spin::Mutex;
use x86_64::PhysAddr;
use x86_64::VirtAddr;
use x86_64::structures::paging::{Page, PageTableFlags, PhysFrame, Size4KiB};

pub fn sys_create_window(context: &mut SyscallContext, args: [u64; 6]) {
    let x = args[0] as usize;
    let y = args[1] as usize;
    let w = args[2] as usize;
    let h = args[3] as usize;
    let out_ptr = args[4] as *mut u64;

    if w == 0 || h == 0 || out_ptr.is_null() {
        context.scratch.rax = u64::MAX;
        return;
    }

    let pixel_bytes = w * h * core::mem::size_of::<mos_display::RGBA>();
    let npages = (align_up(pixel_bytes as u64, 4096) / 4096) as usize;

    let Some(frames) = PhysicalFrameManager.global_allocate_frames(npages) else {
        context.scratch.rax = u64::MAX;
        return;
    };
    let phys_start = frames.start.start_address();

    let kernel_va = phys_to_virt(phys_start);
    let ptr = kernel_va.as_mut_ptr::<mos_display::RGBA>();

    let tcb = current_tcb();
    let pid = tcb.process.pid;
    let Some(user_range) = tcb.process.mem.find_free_area(npages as u64) else {
        context.scratch.rax = u64::MAX;
        return;
    };
    let user_va = user_range.start.start_address().as_u64();

    let mem_flags = MemoryFlags::new(MemoryProtFlags::READ_WRITE, MemoryMapFlags::PRIVATE);
    if tcb.process.mem.addr_space.access(|mm| {
        mm.try_add_area(MemoryArea::new_anon(user_range, mem_flags))
    }).is_err() {
        context.scratch.rax = u64::MAX;
        return;
    }

    let pte_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE;
    let map_result = tcb.process.mem.addr_space.access(|mm| {
        for i in 0..npages {
            let page = Page::<Size4KiB>::containing_address(VirtAddr::new(user_va + (i * 4096) as u64));
            let frame = PhysFrame::<Size4KiB>::containing_address(
                PhysAddr::new(phys_start.as_u64() + (i * 4096) as u64),
            );
            mm.map_page_eagerly(page, frame, pte_flags).map_err(|_| KError::Unsupported)?;
        }
        Ok::<_, KError>(())
    });
    if map_result.is_err() {
        context.scratch.rax = u64::MAX;
        return;
    }

    unsafe { core::ptr::write_bytes(kernel_va.as_mut_ptr::<u8>(), 0, pixel_bytes); }

    let shared = unsafe { SharedSurface::new(w, h, w, ptr) };
    let window = Arc::new(Mutex::new(Window::new(x, y, shared)));
    let window_id = mos_display::register_user_window(window, pid.as_u64());

    unsafe {
        out_ptr.add(0).write_volatile(window_id);
        out_ptr.add(1).write_volatile(user_va);
        out_ptr.add(2).write_volatile(w as u64);
        out_ptr.add(3).write_volatile(h as u64);
        out_ptr.add(4).write_volatile(w as u64);
    }
    context.scratch.rax = 0;
}

pub fn sys_window_mark_dirty(args: [u64; 6]) {
    let window_id = args[0];
    let x = args[1] as usize;
    let y = args[2] as usize;
    let w = args[3] as usize;
    let h = args[4] as usize;
    mos_display::mark_user_window_dirty(window_id, x, y, w, h);
}

pub fn sys_window_sync() {
    mos_display::render_to_framebuffer();
}

pub fn sys_destroy_window(args: [u64; 6]) {
    mos_display::destroy_user_window(args[0]);
}
