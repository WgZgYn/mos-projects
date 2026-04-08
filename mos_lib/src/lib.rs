#![no_std]
#![feature(coerce_unsized)]
#![feature(unsize)]

pub mod align;
pub mod rand;
pub mod sync;
pub mod utils;

use crate::align::PageAligned;
use core::cell::UnsafeCell;
use core::mem::MaybeUninit;

/// only use at kernel boot init
pub struct KernelBootGlobal<T>(UnsafeCell<MaybeUninit<T>>);
impl<T> KernelBootGlobal<T> {
    pub const fn uninit() -> KernelBootGlobal<T> {
        Self(UnsafeCell::new(MaybeUninit::uninit()))
    }
    pub unsafe fn init(&self, init: impl FnOnce() -> T) {
        unsafe {
            self.0.get().write(MaybeUninit::new(init()));
        }
    }
    pub unsafe fn get(&self) -> &T {
        unsafe { (*self.0.get()).assume_init_ref() }
    }
    pub unsafe fn get_mut(&mut self) -> &mut T {
        unsafe { (*self.0.get()).assume_init_mut() }
    }
}
unsafe impl<T> Send for KernelBootGlobal<T> {}
unsafe impl<T> Sync for KernelBootGlobal<T> {}

pub type Frame4KiB = PageAligned<[u8; 1024 * 4]>;
