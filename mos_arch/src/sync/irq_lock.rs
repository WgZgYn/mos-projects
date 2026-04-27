use crate::with_disabled_interrupts;
use core::cell::UnsafeCell;

/// use for single core perCPU data
pub struct IrqLocked<T>(UnsafeCell<T>);

impl<T> IrqLocked<T> {
    pub const unsafe fn new(value: T) -> IrqLocked<T> {
        IrqLocked(UnsafeCell::new(value))
    }
    pub fn access<'a, R>(&'a self, f: impl FnOnce(&'a mut T) -> R) -> R {
        with_disabled_interrupts(|| f(unsafe { &mut *self.0.get() }))
    }
}

unsafe impl<T> Sync for IrqLocked<T> {}
unsafe impl<T> Send for IrqLocked<T> {}
