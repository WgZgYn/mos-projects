use core::cell::UnsafeCell;

pub struct DummyLock<T>(UnsafeCell<T>);
impl<T> DummyLock<T> {
    pub const fn new(value: T) -> DummyLock<T> {
        DummyLock(UnsafeCell::new(value))
    }
    /// # Safety
    /// Only when it's the very early of the kernel, which is running on one thread
    pub unsafe fn access<'a, R>(&'a self, f: impl FnOnce(&'a mut T) -> R) -> R {
        unsafe { f(&mut *self.0.get()) }
    }
}
unsafe impl<T> Sync for DummyLock<T> {}
unsafe impl<T> Send for DummyLock<T> {}
