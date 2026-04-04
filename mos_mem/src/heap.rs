use crate::config::{GLOBAL_HEAP_SIZE, GLOBAL_HEAP_START};
use core::alloc::{GlobalAlloc, Layout};
use core::ptr::NonNull;
use linked_list_allocator::Heap;
use mos_arch::sync::SpinIrqLock;
use x86_64::VirtAddr;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB};

#[global_allocator]
static ALLOCATOR: SpinIrqHeap = SpinIrqHeap::empty();

pub struct SpinIrqHeap(SpinIrqLock<Heap>);
impl SpinIrqHeap {
    pub const fn empty() -> Self {
        Self(SpinIrqLock::new(Heap::empty()))
    }
    pub fn init(&self, heap_bottom: *mut u8, heap_size: usize) {
        self.0
            .access(|heap| unsafe { heap.init(heap_bottom, heap_size) })
    }
    pub fn access<R, F: FnOnce(&mut Heap) -> R>(&self, f: F) -> R {
        self.0.access(f)
    }
}

unsafe impl GlobalAlloc for SpinIrqHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.0.access(|heap| {
            heap.allocate_first_fit(layout)
                .ok()
                .map_or(core::ptr::null_mut(), |allocation| allocation.as_ptr())
        })
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.0
            .access(|heap| unsafe { heap.deallocate(NonNull::new_unchecked(ptr), layout) })
    }
}

pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut (impl FrameAllocator<Size4KiB> + ?Sized),
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(GLOBAL_HEAP_START as u64);
        let heap_end = heap_start + GLOBAL_HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    // TODO: handle error rollback, or just panic
    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe { mapper.map_to(page, frame, flags, frame_allocator)?.flush() };
    }

    ALLOCATOR.init(GLOBAL_HEAP_START as *mut u8, GLOBAL_HEAP_SIZE);
    Ok(())
}
