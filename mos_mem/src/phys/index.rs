use crate::regions::{UsableMemoryRegion, UsableMemoryRegions};
use crate::{phys_to_virt, write_zero_byte_to_phys};
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicU32, Ordering, fence};
use log::info;
use x86_64::PhysAddr;

const PRESERVED: u32 = u32::MAX;
static FRAMES: FrameInfoArray = FrameInfoArray::uninit();

struct FrameInfo {
    refcount: AtomicU32,
}

struct FrameInfoArray {
    frames: UnsafeCell<&'static mut [FrameInfo]>,
}

unsafe impl Send for FrameInfoArray {}
unsafe impl Sync for FrameInfoArray {}
impl FrameInfoArray {
    const fn uninit() -> Self {
        Self {
            frames: UnsafeCell::new(&mut []),
        }
    }
    /// # Safety
    /// must and only call once
    unsafe fn init(&self, info: &'static mut [FrameInfo]) {
        info.iter_mut()
            .for_each(|frame| frame.refcount = AtomicU32::new(PRESERVED));
        unsafe {
            *self.frames.get() = info;
        }
    }
    /// # Safety
    unsafe fn access_mut<F, R>(&self, index: usize, f: F) -> R
    where
        F: FnOnce(&mut FrameInfo) -> R,
    {
        unsafe {
            let info = &mut *self.frames.get();
            f(&mut info[index])
        }
    }
    fn access<F, R>(&self, index: usize, f: F) -> R
    where
        F: FnOnce(&FrameInfo) -> R,
    {
        unsafe {
            let info = &mut *self.frames.get();
            f(&mut info[index])
        }
    }
}

/// the num of Size4KiB Frame
const fn frames_for_frame_info(total_frames: usize) -> usize {
    const FRAME_INFO_STRUCT_SIZE: usize = size_of::<FrameInfo>();
    const FRAME_SIZE: usize = 4096;

    let metadata_bytes = FRAME_INFO_STRUCT_SIZE * total_frames;
    metadata_bytes.div_ceil(FRAME_SIZE)
}

unsafe fn init_frame_info_array(start: *mut FrameInfo, len: usize) {
    unsafe {
        let array = core::slice::from_raw_parts_mut(start, len);
        FRAMES.init(array);
    }
}

/// this method will use `mem` to init the FrameInfo array,
pub fn init(mem: &mut UsableMemoryRegions) {
    let max_frame_count = mem
        .usable_regions()
        .iter()
        .map(|region| region.end >> 12)
        .max()
        .expect("No memory regions found");

    let count = frames_for_frame_info(max_frame_count as _);
    let target_region = mem
        .usable_regions()
        .iter()
        .enumerate()
        .find(|(_, region)| region.frames_count() >= count);

    let start: u64;
    let end: u64;
    if let Some((i, region)) = target_region {
        start = region.start;
        end = start + count as u64 * 4096;
        let new_region = UsableMemoryRegion::from_exactly(end, region.end).unwrap();
        mem.shrink(i, new_region);
    } else {
        panic!("currently only support init at continuous physical memory region");
    }

    let paddr = PhysAddr::new(start);
    unsafe {
        write_zero_byte_to_phys(paddr, (end - start) as _);
        let vaddr = phys_to_virt(paddr);
        init_frame_info_array(vaddr.as_mut_ptr(), max_frame_count as _);

        mem.usable_regions()
            .iter()
            .flat_map(|region| (region.start >> 12)..(region.end >> 12))
            .for_each(|pn| {
                FRAMES.access_mut(pn as usize, |info| info.refcount = AtomicU32::new(0));
            });
    }
}

pub fn alloc_frame(pn: usize) -> bool {
    FRAMES.access(pn, |info| {
        info.refcount
            .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
    })
}

/// return old value, if return 0 means
pub fn inc_refcount(pn: usize) -> u32 {
    FRAMES.access(pn, |info| info.refcount.fetch_add(1, Ordering::Relaxed))
}

/// do this when refcount dec down to 0
pub fn dec_refcount_and_if_zero<F: FnOnce()>(pn: usize, f: F) -> u32 {
    FRAMES.access(pn, |info| {
        let old = info.refcount.fetch_sub(1, Ordering::Release);
        if old == 1 {
            fence(Ordering::Acquire);
            f();
        }
        old
    })
}

pub fn get_refcount(pn: usize) -> u32 {
    FRAMES.access(pn, |info| info.refcount.load(Ordering::Acquire))
}

pub fn debug_info() {
    let array = unsafe { &*FRAMES.frames.get() };
    info!("Frame count: {}", array.len());
}
