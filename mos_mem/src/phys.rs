mod buddy;
mod index;

use super::phys::buddy::{global_buddy, init_buddy};
use super::phys::index::{alloc_frame, dec_refcount_and_if_zero, get_refcount, inc_refcount};
use super::regions::UsableMemoryRegions;
use super::zero_frame;
use x86_64::structures::paging::{FrameAllocator, FrameDeallocator, PhysFrame, Size4KiB};

#[derive(Default, Copy, Clone)]
pub struct PhysicalFrameManager;

unsafe impl FrameAllocator<Size4KiB> for PhysicalFrameManager {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.global_allocate_frame()
    }
}

impl FrameDeallocator<Size4KiB> for PhysicalFrameManager {
    unsafe fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        self.ref_dec(frame);
    }
}

impl PhysicalFrameManager {
    pub fn init(regions: &mut UsableMemoryRegions) {
        index::init(regions);
        index::debug_info();
        init_buddy(regions.usable_regions())
    }

    pub fn global_allocate_frame(self) -> Option<PhysFrame<Size4KiB>> {
        global_buddy().allocate_frame().inspect(|frame| {
            let pn = frame.start_address().as_u64() >> 12;
            let ok = alloc_frame(pn as usize);
            assert!(ok, "alloc frame, the old refcount must be 0");
            zero_frame(*frame);
        })
    }

    pub fn ref_inc(self, frame: PhysFrame<Size4KiB>) {
        let old = inc_refcount((frame.start_address().as_u64() >> 12) as usize);

        // avoid use before alloc
        assert!(old > 0);
    }
    pub fn ref_dec(self, frame: PhysFrame<Size4KiB>) -> bool {
        let old = dec_refcount_and_if_zero((frame.start_address().as_u64() >> 12) as usize, || {
            global_buddy().deallocate_frame(frame);
        });

        // avoid underflow
        assert_ne!(old, 0);

        old == 1
    }

    pub fn ref_get(self, frame: PhysFrame<Size4KiB>) -> u32 {
        get_refcount((frame.start_address().as_u64() >> 12) as usize)
    }

    // // TODO
    // pub fn global_allocate_frames(self, n: usize) -> Option<PhysFrameRange<Size4KiB>> {
    //     BUDDY.allocate_frames(n).inspect(|frames| {
    //         frames.for_each(|frame| {
    //             let pn = frame.start_address().as_u64() >> 12;
    //             let ok = alloc_frame(pn as usize);
    //             assert!(ok, "alloc frame, the old refcount must be 0");
    //             zero_frame(frame)
    //         })
    //     })
    // }
    //
    // // TODO:
    // pub fn global_deallocate_frames(self, frames: PhysFrameRange<Size4KiB>) {
    //     BUDDY.deallocate_frames(frames);
    // }
}
