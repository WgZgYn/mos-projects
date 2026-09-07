use mos_abi::error::KResult;
use crate::config::BUDDY_METADATA_HEAP_SIZE;
use crate::heap::SpinIrqHeap;
use crate::regions::UsableMemoryRegion;
use buddy_allocator::FrameAllocator;
use core::alloc::{AllocError, Allocator, Layout};
use core::ops::Deref;
use core::ptr::NonNull;
use mos_arch::sync::SpinIrqLock;
use mos_lib::KernelBootGlobal;
use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::{PhysFrame, Size4KiB};

static BUDDY: KernelBootGlobal<BuddyFrameManager> = KernelBootGlobal::uninit();

pub fn init_buddy(regions: &[UsableMemoryRegion]) {
    unsafe { BUDDY.init(|| BuddyFrameManager::new_init(regions)) }
}

pub fn global_buddy() -> &'static BuddyFrameManager {
    unsafe { BUDDY.get() }
}

pub struct BuddyFrameManager {
    allocator: SpinIrqLock<FrameAllocator<32, BuddyBoostAllocator>>,
}

impl BuddyFrameManager {
    pub fn new_init(regions: &[UsableMemoryRegion]) -> Self {
        let buddy = Self {
            allocator: SpinIrqLock::new(FrameAllocator::new_in(
                init_buddy_boost_allocator().expect("BuddyFrameManager memory preparation failed"),
            )),
        };

        buddy.allocator.access(|buddy| {
            for region in regions {
                buddy.add_frame((region.start >> 12) as usize, (region.end >> 12) as usize)
            }
        });

        buddy
    }

    pub fn allocate_frame(&self) -> Option<PhysFrame<Size4KiB>> {
        self.allocator.access(|allocator| {
            allocator.alloc(1).map(|n| unsafe {
                PhysFrame::from_start_address_unchecked(PhysAddr::new((n << 12) as _))
            })
        })
    }

    pub fn deallocate_frame(&self, frame: PhysFrame<Size4KiB>) {
        self.allocator.access(|allocator| {
            allocator.dealloc(frame.start_address().as_u64() as usize >> 12, 1);
        })
    }

    pub fn allocate_frames(&self, count: usize) -> Option<PhysFrameRange<Size4KiB>> {
        self.allocator.access(|allocator| {
            allocator.alloc(count).map(|start| PhysFrameRange {
                start: PhysFrame::from_start_address(PhysAddr::new((start as u64) << 12))
                    .expect("should aligned"),
                end: PhysFrame::from_start_address(PhysAddr::new(((start + count) as u64) << 12))
                    .expect("should aligned"),
            })
        })
    }
    pub fn deallocate_frames(&self, range: PhysFrameRange<Size4KiB>) {
        self.allocator.access(|allocator| {
            allocator.dealloc(
                range.start.start_address().as_u64() as usize >> 12,
                range.len() as usize,
            );
        });
    }
}

#[derive(Clone)]
struct BuddyBoostAllocator;
impl Deref for BuddyBoostAllocator {
    type Target = SpinIrqHeap;
    fn deref(&self) -> &Self::Target {
        static METADATA: SpinIrqHeap = SpinIrqHeap::empty();
        &METADATA
    }
}
unsafe impl Allocator for BuddyBoostAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 {
            return Ok(NonNull::slice_from_raw_parts(layout.dangling_ptr(), 0));
        }
        self.deref()
            .access(|heap| match heap.allocate_first_fit(layout) {
                Ok(ptr) => Ok(NonNull::slice_from_raw_parts(ptr, layout.size())),
                Err(()) => Err(AllocError),
            })
    }
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        unsafe {
            if layout.size() != 0 {
                self.deref().access(|heap| heap.deallocate(ptr, layout));
            }
        }
    }
}

#[allow(static_mut_refs)]
fn init_buddy_boost_allocator() -> KResult<BuddyBoostAllocator> {
    let boost: BuddyBoostAllocator = BuddyBoostAllocator;
    static mut BUDDY_META: [u8; BUDDY_METADATA_HEAP_SIZE as usize] =
        [0; BUDDY_METADATA_HEAP_SIZE as usize];
    unsafe {
        boost.init(BUDDY_META.as_mut_ptr(), BUDDY_METADATA_HEAP_SIZE as _);
    }
    Ok(boost)
}
