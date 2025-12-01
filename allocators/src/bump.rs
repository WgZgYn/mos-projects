use crate::{Locked, align_up};
use core::alloc::{GlobalAlloc, Layout};

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
    allocations: usize,
}

impl BumpAllocator {
    pub const fn new() -> BumpAllocator {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
        self.allocations = 0;
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut guard = self.inner.lock();
        let alloc_start = align_up(guard.next, layout.align());
        let Some(alloc_end) = alloc_start.checked_add(layout.size()) else {
            return core::ptr::null_mut();
        };
        if alloc_end > guard.heap_end {
            return core::ptr::null_mut();
        }
        guard.next = alloc_end;
        guard.allocations += 1;
        alloc_start as *mut u8
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut guard = self.inner.lock();
        if guard.allocations <= 1 {
            guard.allocations = 0;
            guard.next = guard.heap_start;
        } else {
            guard.allocations -= 1;
        }
    }
}
