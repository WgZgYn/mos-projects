mod tests {
    use std::alloc::{GlobalAlloc, Layout};

    #[test]
    fn test_new() {
        let mut bs = [0; 4096];
        let mut a = linked_list_allocator::Heap::empty();
        unsafe {
            a.init(bs.as_mut_ptr(), 4096);
        }
    }

    #[test]
    fn test_alloc() {
        let mut bs = [0; 4096];
        let a = linked_list_allocator::LockedHeap::empty();
        unsafe {
            a.lock().init(bs.as_mut_ptr(), 4096);
            let p1 = a.alloc(Layout::new::<i32>());
            let p2 = a.alloc(Layout::new::<i32>());
            assert!(!p1.is_null());
            assert!(!p2.is_null());
            assert_ne!(p1, p2);
        }
    }

    #[test]
    fn test_dealloc() {
        let mut bs = [0; 4096];
        let a = linked_list_allocator::LockedHeap::empty();
        unsafe {
            a.lock().init(bs.as_mut_ptr(), 4096);
            let p1 = a.alloc(Layout::new::<i32>());
            let p2 = a.alloc(Layout::new::<i32>());
            assert!(!p1.is_null());
            assert!(!p2.is_null());
            assert_ne!(p1, p2);
            a.dealloc(p2, Layout::new::<i32>());
            assert_eq!(p2, a.alloc(Layout::new::<i32>()));
        }
    }
}
