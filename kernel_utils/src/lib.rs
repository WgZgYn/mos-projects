#![no_std]

pub use bitmap::BitMap;
pub use log::{debug, error, info, trace, warn};
pub use num_enum::FromPrimitive;
pub use spin::Mutex;

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



