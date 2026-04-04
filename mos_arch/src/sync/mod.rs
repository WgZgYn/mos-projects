mod irq_lock;
mod spin_irq_lock;

pub use irq_lock::IrqLocked;
pub use spin_irq_lock::{SpinIrqLock, SpinIrqLockGuard};
