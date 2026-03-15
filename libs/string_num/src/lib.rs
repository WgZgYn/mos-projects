#![no_std]

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
