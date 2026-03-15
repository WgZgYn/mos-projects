#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

#[cfg(feature = "alloc")]
use core::sync::atomic::{AtomicU64, Ordering};

use core::ops::{Deref, DerefMut};
use spin::Mutex;

pub const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

pub const fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}

pub const fn is_aligned(addr: u64, align: u64) -> bool {
    addr & (align - 1) == 0
}

#[derive(Clone, Default)]
#[repr(align(4096))]
pub struct PageAligned<T>(pub T);
impl<T> Deref for PageAligned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T> DerefMut for PageAligned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "alloc")]
pub struct IDAllocator {
    next: AtomicU64,
    reuse: Mutex<Vec<u64>>,
}

#[cfg(feature = "alloc")]
impl IDAllocator {
    pub const fn new(start: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
            reuse: Mutex::new(Vec::new()),
        }
    }
    pub fn allocate(&self) -> u64 {
        self.reuse
            .lock()
            .pop()
            .unwrap_or(self.next.fetch_add(1, Ordering::Relaxed))
    }
    pub fn free(&self, id: u64) {
        self.reuse.lock().push(id);
    }
}

#[derive(Default)]
pub struct SpinLockedSession {
    mutex: Mutex<()>,
}

impl SpinLockedSession {
    pub const fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
        }
    }
    pub fn access<R>(&self, f: impl FnOnce() -> R) {
        let guard = self.mutex.lock();
        f();
        drop(guard);
    }
}

impl Deref for SpinLockedSession {
    type Target = Mutex<()>;
    fn deref(&self) -> &Self::Target {
        &self.mutex
    }
}

impl DerefMut for SpinLockedSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mutex
    }
}

pub struct Array<T, const N: usize> {
    data: [Option<T>; N],
    len: usize,
}
impl<T, const N: usize> Default for Array<T, N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T, const N: usize> Array<T, N> {
    pub const fn new() -> Self {
        Self {
            data: [const { None }; N],
            len: 0,
        }
    }
    pub fn push(&mut self, value: T) {
        if self.len < N {
            self.data[self.len] = Some(value);
            self.len += 1;
        } else {
            panic!("out of bounds");
        }
    }
    pub fn pop(&mut self) -> Option<T> {
        if self.len > 0 {
            self.len -= 1;
            self.data[self.len].take()
        } else {
            None
        }
    }
    pub fn len(&self) -> usize {
        self.len
    }
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

pub struct Defer<F: FnOnce()> {
    f: Option<F>,
}

impl<F: FnOnce()> Defer<F> {
    pub fn new(f: F) -> Self {
        Defer { f: Some(f) }
    }

    // 取消执行
    pub fn cancel(&mut self) {
        self.f = None;
    }
}

impl<F: FnOnce()> Drop for Defer<F> {
    fn drop(&mut self) {
        if let Some(f) = self.f.take() {
            f();
        }
    }
}
