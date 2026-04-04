use core::ops::{Deref, DerefMut};

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

#[inline(always)]
pub const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}
#[inline(always)]
pub const fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}
#[inline(always)]
pub const fn is_aligned(addr: u64, align: u64) -> bool {
    addr & (align - 1) == 0
}
