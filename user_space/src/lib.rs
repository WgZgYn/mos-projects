#![no_std]
#![no_main]
#![feature(linkage)]

use crate::syscall::{sys_exit, sys_rand, sys_write, sys_yield};
use core::alloc::{GlobalAlloc, Layout};
use core::ops::Deref;
use linked_list_allocator::LockedHeap;

pub mod console;
mod syscall;

fn init_heap() -> LockedHeap {
    static mut DATA: [u8; 4096] = [0; 4096];
    let heap = unsafe { LockedHeap::new(&raw mut DATA as _, 4096) };
    heap
}

pub struct HEAP {
    __private_field: (),
}

#[global_allocator]
pub static HEAP: HEAP = HEAP {
    __private_field: (),
};
impl ::lazy_static::__Deref for HEAP {
    type Target = LockedHeap;
    fn deref(&self) -> &LockedHeap {
        #[inline(always)]
        fn __static_ref_initialize() -> LockedHeap {
            init_heap()
        }

        #[inline(always)]
        fn __stability() -> &'static LockedHeap {
            static LAZY: ::lazy_static::lazy::Lazy<LockedHeap> = ::lazy_static::lazy::Lazy::INIT;
            LAZY.get(__static_ref_initialize)
        }
        __stability()
    }
}
impl ::lazy_static::LazyStatic for HEAP {
    fn initialize(lazy: &Self) {
        let _ = &**lazy;
    }
}
unsafe impl GlobalAlloc for HEAP {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { self.deref().alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.deref().dealloc(ptr, layout) }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() {
    exit(main());
}

#[linkage = "weak"]
#[unsafe(no_mangle)]
fn main() -> u64 {
    panic!("no main")
}

#[panic_handler]
fn panic_handler(panic_info: &core::panic::PanicInfo) -> ! {
    let err = panic_info.message();
    if let Some(location) = panic_info.location() {
        println!(
            "Panicked at {}:{}, {}",
            location.file(),
            location.line(),
            err
        );
    } else {
        println!("Panicked: {}", err);
    }
    loop {}
}

pub const STDOUT: u64 = 1;

pub fn write(fd: u64, buffer: &[u8]) -> u64 {
    sys_write(fd, buffer)
}

pub fn exit(code: u64) -> u64 {
    sys_exit(code)
}

pub fn rand() -> u64 {
    sys_rand()
}

pub fn yield_() -> u64 {
    sys_yield()
}
