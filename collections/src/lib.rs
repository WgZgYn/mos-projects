#![no_std]

pub mod tree;

#[macro_export]
macro_rules! container_of {
    ($ptr:expr, $ty:ty, $member:tt) => {{
        ($ptr as *const u8)
            .sub(core::mem::offset_of!($ty, $member))
            as *const $ty
    }};
}

/// # Safety
/// impl must ensure the mem safety
pub unsafe trait ContainerOf<Tag> {
    type Node;
    fn container_of(ptr: *const Self::Node) -> *const Self;
}