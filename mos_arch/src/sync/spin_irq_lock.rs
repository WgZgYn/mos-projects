use crate::{
    disable_interrupts, enable_interrupts, is_enable_interrupts, with_disabled_interrupts,
};
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::drop_in_place;

pub struct SpinIrqLock<T> {
    inner: spin::Mutex<T>,
}

impl<T> SpinIrqLock<T> {
    pub const fn new(value: T) -> SpinIrqLock<T> {
        Self {
            inner: spin::Mutex::new(value),
        }
    }
    pub fn access<R, F: FnOnce(&mut T) -> R>(&self, f: F) -> R {
        with_disabled_interrupts(|| {
            let mut guard = self.inner.lock();
            f(guard.deref_mut())
        })
    }

    pub fn lock(&self) -> SpinIrqLockGuard<'_, T> {
        SpinIrqLockGuard::new(self)
    }
}

pub struct SpinIrqLockGuard<'a, T: 'a> {
    guard: ManuallyDrop<spin::MutexGuard<'a, T>>,
    prev: bool,
}

impl<'a, T> SpinIrqLockGuard<'a, T> {
    fn new(lock: &'a SpinIrqLock<T>) -> SpinIrqLockGuard<'a, T> {
        let prev = is_enable_interrupts();
        disable_interrupts();
        Self {
            guard: ManuallyDrop::new(lock.inner.lock()),
            prev,
        }
    }
}

impl<'a, T> Drop for SpinIrqLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            drop_in_place(self.guard.deref_mut());
        }
        if self.prev {
            enable_interrupts();
        } else {
            disable_interrupts();
        }
    }
}

impl<'a, T> Deref for SpinIrqLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.guard.deref()
    }
}

impl<'a, T> DerefMut for SpinIrqLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.guard.deref_mut()
    }
}
