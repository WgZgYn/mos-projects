use crate::scheduler::{dec_preempt_count, inc_preempt_count};
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::ptr::drop_in_place;

pub struct PreemptDisabled<T>(T);
impl<T> PreemptDisabled<T> {
    pub const fn new(val: T) -> Self {
        Self(val)
    }
    pub fn access<R>(&self, f: impl FnOnce(&T) -> R) -> R {
        inc_preempt_count();
        let r = f(&self.0);
        dec_preempt_count();
        r
    }
    pub fn access_mut<R>(&mut self, f: impl FnOnce(&mut T) -> R) -> R {
        inc_preempt_count();
        let r = f(&mut self.0);
        dec_preempt_count();
        r
    }
}

pub struct PreemptSpinLock<T>(spin::Mutex<T>);
impl<T> PreemptSpinLock<T> {
    pub const fn new(val: T) -> Self {
        Self(spin::Mutex::new(val))
    }
    pub fn lock(&'_ self) -> PreemptSpinLockGuard<'_, T> {
        PreemptSpinLockGuard::new(self)
    }
}
pub struct PreemptSpinLockGuard<'a, T>(ManuallyDrop<spin::MutexGuard<'a, T>>);
impl<'a, T> Drop for PreemptSpinLockGuard<'a, T> {
    fn drop(&mut self) {
        unsafe { drop_in_place(self.0.deref_mut()) }
        dec_preempt_count();
    }
}
impl<'a, T> PreemptSpinLockGuard<'a, T> {
    pub fn new(lock: &'a PreemptSpinLock<T>) -> Self {
        inc_preempt_count();
        Self(ManuallyDrop::new(lock.0.lock()))
    }
}
impl<'a, T> Deref for PreemptSpinLockGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}
impl<'a, T> DerefMut for PreemptSpinLockGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.deref_mut()
    }
}
