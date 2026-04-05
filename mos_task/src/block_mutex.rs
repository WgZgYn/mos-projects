use crate::scheduler::{current_tcb, schedule};
use crate::wait_queue::WaitQueue;
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};
use crate::thread::ThreadStateValue;

pub struct Mutex<T> {
    data: UnsafeCell<T>,
    lock: AtomicBool,
    wait_queue: WaitQueue,
}

pub struct MutexGuard<'a, T> {
    inner: &'a Mutex<T>,
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        self.inner.lock.store(false, Ordering::Release);
        self.inner.wait_queue.wake_one(); // or wake_all ?, or I need use semp
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.inner.data.get() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.inner.data.get() }
    }
}

impl<T> Mutex<T> {
    /// caller must make sure it's safe to schedule
    /// maybe schedule
    pub fn lock(&self) -> MutexGuard<'_, T> {
        loop {
            let mut queue = self.wait_queue.lock();
            if let Some(guard) = self.try_lock() {
                return guard;
            } else {
                let current = current_tcb();
                current.set_state(ThreadStateValue::Blocked);
                queue.deref_mut().enqueue(current);
                drop(queue);
                schedule()
            }
        }
    }
    pub fn try_lock(&self) -> Option<MutexGuard<'_, T>> {
        if self
            .lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_ok()
        {
            Some(MutexGuard { inner: self })
        } else {
            None
        }
    }
}
