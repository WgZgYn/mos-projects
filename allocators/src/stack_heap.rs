use core::alloc::{GlobalAlloc, Layout};
use crate::{align_up, Locked};

// 标志位操作
const FLAG_BIT: usize = usize::BITS as usize - 1;
const FLAG_MASK: usize = 1 << FLAG_BIT;
const PTR_MASK: usize = !FLAG_MASK;

#[inline]
const fn set_allocated(size: usize) -> usize {
    size | FLAG_MASK
}

#[inline]
const fn set_free(size: usize) -> usize {
    size & PTR_MASK
}

#[inline]
const fn is_allocated(size_with_flag: usize) -> bool {
    (size_with_flag & FLAG_MASK) != 0
}

#[inline]
const fn get_ptr(size_with_flag: usize) -> usize {
    size_with_flag & PTR_MASK
}

// 块头结构
#[repr(C)]
struct BlockHeader {
    prev_header_and_flag: usize,
}

impl BlockHeader {
    #[inline]
    fn new(prev_header: usize, allocated: bool) -> Self {
        Self {
            prev_header_and_flag: if allocated {
                set_allocated(prev_header)
            } else {
                set_free(prev_header)
            },
        }
    }

    #[inline]
    fn is_allocated(&self) -> bool {
        is_allocated(self.prev_header_and_flag)
    }

    #[inline]
    fn set_allocated(&mut self, allocated: bool) {
        if allocated {
            self.prev_header_and_flag |= FLAG_MASK;
        } else {
            self.prev_header_and_flag &= PTR_MASK;
        }
    }

    #[inline]
    fn prev_header(&self) -> usize {
        get_ptr(self.prev_header_and_flag)
    }
}

pub struct StackHeap {
    heap_start: usize,
    heap_end: usize,
    last_block: usize,
}

impl StackHeap {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            last_block: 0,
        }
    }

    pub fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.last_block = heap_start;

        // 初始化第一个块头作为哨兵
        unsafe {
            let header = heap_start as *mut BlockHeader;
            header.write(BlockHeader::new(0, true)); // 哨兵块，始终分配
        }
    }
}

unsafe impl GlobalAlloc for Locked<StackHeap> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut heap = self.lock();
        let user_ptr = align_up(heap.last_block + size_of::<BlockHeader>(), layout.align());
        let header_ptr = align_up(user_ptr + layout.size(), align_of::<BlockHeader>());
        unsafe {
            (header_ptr as *mut BlockHeader).write(BlockHeader::new(heap.last_block, true));
        }
        heap.last_block = header_ptr;
        user_ptr as *mut u8
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let header_ptr = align_up(ptr as usize + layout.size(), align_of::<BlockHeader>());
        assert!(header_ptr as usize <= self.inner.lock().heap_end);
        unsafe {
            (*(header_ptr as *mut BlockHeader)).set_allocated(false);
        }
        // ptr.write_bytes(0, layout.size());
        let mut heap = self.lock();
        while heap.last_block > heap.heap_start {
            let curr_header = unsafe { &*(heap.last_block as *mut BlockHeader) };
            if !curr_header.is_allocated() {
                heap.last_block = curr_header.prev_header();
                // (curr_header as *const BlockHeader as *mut BlockHeader)
                //     .write_bytes(0, size_of::<BlockHeader>());
            } else {
                break;
            }
        }
    }
}
