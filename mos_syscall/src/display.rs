//! Process-owned display handles. The compositor pins backing frames separately
//! from user PTEs so address-space teardown cannot leave dangling pixel pointers.
use alloc::{boxed::Box, sync::Arc};
use mos_abi::error::{KError, KResult};
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_arch::structs::SyscallContext;
use mos_display::window::{SharedSurface, Window};
use mos_mem::virt::{MemoryArea, MemoryFlags};
use mos_mem::{PhysicalFrameManager, phys_to_virt};
use mos_task::current_tcb;
use spin::Mutex;
use x86_64::structures::paging::{PageTableFlags, frame::PhysFrameRange};

struct BackingFrames(PhysFrameRange);
impl Drop for BackingFrames {
    fn drop(&mut self) { PhysicalFrameManager.global_deallocate_frames(self.0); }
}

struct WindowHandle { id: u64, pid: u64 }
impl mos_task::process::ImageResource for WindowHandle {
    fn key(&self) -> (&'static str, u64) { ("window", self.id) }
}
impl Drop for WindowHandle {
    fn drop(&mut self) { mos_display::destroy_user_window(self.id, self.pid); }
}

pub fn sys_create_window(context: &mut SyscallContext, args: [u64; 6]) {
    let result = create_window(args);
    // Preparing/copying the result can resolve a COW destination page.
    mos_platform::smp::tlb_shootdown();
    match result {
        Ok(()) => context.scratch.rax = 0,
        Err(error) => crate::ret_err(context, error),
    }
}

fn create_window(args: [u64; 6]) -> KResult<()> {
    let [x, y, w, h, output, _] = args;
    // Bound allocation and all geometry before arithmetic or frame allocation.
    if w == 0 || h == 0 || w > 4096 || h > 4096
        || x.checked_add(w).is_none() || y.checked_add(h).is_none() {
        return Err(KError::InvalidArg);
    }
    let tcb = current_tcb();
    let pcb = &tcb.process;
    pcb.mem.validate_user_write(output, 40)?;
    let bytes = w * h * 4;
    let pages = bytes.div_ceil(4096) as usize;
    let range = pcb.mem.find_free_area(pages as u64).ok_or(KError::OutOfMem)?;
    let backing = Arc::new(BackingFrames(PhysicalFrameManager.global_allocate_frames(pages)
        .ok_or(KError::OutOfMem)?));
    let flags = MemoryFlags::new(MemoryProtFlags::READ_WRITE, MemoryMapFlags::SHARED);
    pcb.mem.addr_space.access(|mm| mm.try_add_area(MemoryArea::new_anon(range, flags)))?;
    let mapped = pcb.mem.addr_space.access(|mm| {
        for (page, frame) in range.zip(backing.0) {
            if mm.map_page_eagerly(page, frame, PageTableFlags::PRESENT
                | PageTableFlags::WRITABLE | PageTableFlags::USER_ACCESSIBLE
                | PageTableFlags::NO_EXECUTE).is_err() {
                mm.rollback_eager_area(range.start, pages);
                return Err(KError::OutOfMem);
            }
            // Initial allocation belongs to BackingFrames; the PTE has its own ref.
            PhysicalFrameManager.ref_inc(frame);
        }
        Ok(())
    });
    mapped?;
    let ptr = phys_to_virt(backing.0.start.start_address()).as_mut_ptr();
    let surface = unsafe { SharedSurface::new(w as usize, h as usize, w as usize, ptr, backing) };
    let pid = pcb.pid.as_u64();
    let id = mos_display::register_user_window(
        Arc::new(Mutex::new(Window::new(x as usize, y as usize, surface))), pid);
    let handle = Box::new(WindowHandle { id, pid });
    let mut result = [0u8; 40];
    for (chunk, value) in result.chunks_exact_mut(8)
        .zip([id, range.start.start_address().as_u64(), w, h, w]) {
        chunk.copy_from_slice(&value.to_ne_bytes());
    }
    if let Err(error) = pcb.mem.copy_to_user(output, &result) {
        drop(handle);
        pcb.mem.addr_space.access(|mm| mm.rollback_eager_area(range.start, pages));
        return Err(error);
    }
    pcb.image_resources.lock().push(handle);
    Ok(())
}

pub fn sys_window_mark_dirty(context: &mut SyscallContext, args: [u64; 6]) {
    if !mos_display::mark_user_window_dirty(args[0], current_tcb().process.pid.as_u64(),
        args[1] as usize, args[2] as usize, args[3] as usize, args[4] as usize) {
        crate::ret_err(context, KError::NoFile);
    }
}

pub fn sys_window_sync() { mos_display::render_to_framebuffer(); }

pub fn sys_destroy_window(context: &mut SyscallContext, args: [u64; 6]) {
    if !mos_display::destroy_user_window(args[0], current_tcb().process.pid.as_u64()) {
        crate::ret_err(context, KError::NoFile);
    } else {
        current_tcb().process.image_resources.lock()
            .retain(|resource| resource.key() != ("window", args[0]));
    }
}
