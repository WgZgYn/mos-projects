#![feature(allocator_api)]
#![feature(btreemap_alloc)]
#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "use_spin")]
extern crate spin;
#[cfg(test)]
#[macro_use]
extern crate std;

#[cfg(feature = "alloc")]
mod frame;

#[cfg(feature = "alloc")]
pub use frame::*;

pub(crate) fn prev_power_of_two(num: usize) -> usize {
    1 << (usize::BITS as usize - num.leading_zeros() as usize - 1)
}
