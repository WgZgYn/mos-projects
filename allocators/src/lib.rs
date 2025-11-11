mod bump;
mod stack_heap;

fn align_up(addr: usize, align: usize) -> usize {
    (addr + align - 1) & !(align - 1)
}
pub struct Locked<A> {
    inner: spin::Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Self {
            inner: spin::Mutex::new(inner),
        }
    }
    pub fn lock(&self) -> spin::MutexGuard<'_, A> {
        self.inner.lock()
    }
}
