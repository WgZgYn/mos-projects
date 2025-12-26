#![no_std]
extern crate alloc;

use alloc::vec::Vec;
pub use bitflags::bitflags;
pub use bitmap::BitMap;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicU64, Ordering};
pub use log::{debug, error, info, trace, warn};
pub use num_enum::FromPrimitive;
pub use spin::*;

pub const fn align_up(addr: u64, align: u64) -> u64 {
    (addr + align - 1) & !(align - 1)
}

pub const fn align_down(addr: u64, align: u64) -> u64 {
    addr & !(align - 1)
}

pub const fn is_aligned(addr: u64, align: u64) -> bool {
    addr & (align - 1) == 0
}

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
#[macro_export]
macro_rules! string_enum {
    (
        $($(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($variant:ident $(= $val:expr)?),* $(,)?
        })*
    ) => {
        $($(#[$meta])*
        $vis enum $name {
            $($variant $(= $val)?),*
        }

        impl $name {
            pub const fn as_str(&self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant$(($val))?),)*
                }
            }

            pub fn from_str(s: &str) -> Option<Self> {
                match s {
                    $(stringify!($variant) => Some(Self::$variant),)*
                    _ => None,
                }
            }

            pub const fn all_variants() -> &'static [Self] {
                &[$(Self::$variant),*]
            }

            pub fn iter() -> impl Iterator<Item = Self> {
                [$(Self::$variant),*].into_iter()
            }
        }

        impl core::fmt::Display for $name {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(f, "{}", self.as_str())
            }
        }

        impl core::str::FromStr for $name {
            type Err = &'static str;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::from_str(s).ok_or("Invalid variant")
            }
        })*
    };
}

//
#[macro_export]
macro_rules! struct_field_public {
    (
        $($(#[$struct_meta:meta])*
        $vis:vis struct $name:ident $(<
            $($generic:ident $(: $bound:tt $(+ $more_bounds:tt)* )? ),* $(,)?
        >)? {
            $(
                $(#[$field_meta:meta])*
                $_vis:vis $field:ident: $ty:ty
            ),* $(,)?
        })*
    ) => {
        $($(#[$struct_meta])*
        $vis struct $name $(<
            $($generic $(: $bound $(+ $more_bounds)* )? ),*
        >)? {
            $(
                $(#[$field_meta])*
                pub $field: $ty
            ),*
        })*
    };
}

#[macro_export]
macro_rules! tuple_field_public {
    (
        $(#[$struct_meta:meta])*
        $vis:vis struct $name:ident $(<
            $($generic:ident $(: $bound:tt $(+ $more_bounds:tt)* )? ),* $(,)?
        >)? (
            $(
                $(#[$field_meta:meta])*
                $ty:ty
            ),* $(,)?);
    ) => {
        $(#[$struct_meta])*
        $vis struct $name $(<
            $($generic $(: $bound $(+ $more_bounds)* )? ),*
        >)? ($($($field_meta)* pub $ty,)*);
    };
}

#[macro_export]
macro_rules! func_public {
    () => {};
}

#[macro_export]
macro_rules! impl_func_public {
    () => {};
}

pub struct IDAllocator {
    next: AtomicU64,
    reuse: Mutex<Vec<u64>>,
}

impl IDAllocator {
    pub const fn new(start: u64) -> Self {
        Self {
            next: AtomicU64::new(start),
            reuse: Mutex::new(Vec::new()),
        }
    }
    pub fn allocate(&self) -> u64 {
        self.reuse
            .lock()
            .pop()
            .unwrap_or(self.next.fetch_add(1, Ordering::Relaxed))
    }
    pub fn free(&self, id: u64) {
        self.reuse.lock().push(id);
    }
}

#[derive(Default)]
pub struct SpinLockedSession {
    mutex: Mutex<()>,
}

impl SpinLockedSession {
    pub const fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
        }
    }
    pub fn access<R>(&self, f: impl FnOnce() -> R) {
        let guard = self.mutex.lock();
        f();
        drop(guard);
    }
}

impl Deref for SpinLockedSession {
    type Target = Mutex<()>;
    fn deref(&self) -> &Self::Target {
        &self.mutex
    }
}

impl DerefMut for SpinLockedSession {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.mutex
    }
}
