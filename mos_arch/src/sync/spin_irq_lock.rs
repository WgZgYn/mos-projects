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

    /// # Safety
    /// Exactly one live guard must have been consumed by handoff_for_switch.
    /// The caller must no longer access its data and must keep IRQs disabled.
    pub unsafe fn unlock_after_switch(&self) {
        unsafe { self.inner.force_unlock(); }
    }
}

pub struct SpinIrqLockGuard<'a, T: 'a> {
    guard: ManuallyDrop<spin::MutexGuard<'a, T>>,
    prev: bool,
}

impl<'a, T> SpinIrqLockGuard<'a, T> {
    /// Transfer unlock responsibility to the stack-switch assembly. The
    /// suspended caller restores its own IRQ state only when it resumes.
    /// # Safety
    /// Must unlock exactly once after leaving the outgoing task's stack.
    pub unsafe fn handoff_for_switch(self) -> bool {
        let prev = self.prev;
        core::mem::forget(self);
        prev
    }

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
