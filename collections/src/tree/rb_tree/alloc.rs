extern crate alloc;

use crate::tree::rb_tree::RBNodeColor;
use crate::tree::rb_tree::RedBlackNode;
use crate::tree::BinaryNode;
use crate::tree::ParentRefNode;
use crate::{impl_binary_node, impl_parent_node, impl_rb_node};
use core::cmp::Ordering;
use core::ptr::null_mut;

pub struct RBNode<T> {
    parent: *mut RBNode<T>,
    left: *mut RBNode<T>,
    right: *mut RBNode<T>,
    color: RBNodeColor,
    key: T,
}

impl_binary_node!(RBNode<T>);
impl_parent_node!(RBNode<T>);
impl_rb_node!(RBNode<T>);

impl<T: Ord> Eq for RBNode<T> {}

impl<T: Ord> PartialEq<Self> for RBNode<T> {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

impl<T: Ord> PartialOrd<Self> for RBNode<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T: Ord> Ord for RBNode<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.key.cmp(&other.key)
    }
}

impl<T> RBNode<T> {
    pub const fn new(key: T, color: RBNodeColor) -> Self {
        Self {
            parent: null_mut(),
            left: null_mut(),
            right: null_mut(),
            color,
            key,
        }
    }
    pub fn default() -> Self
    where
        T: Default,
    {
        Self::new(T::default(), RBNodeColor::Red)
    }
    pub const fn as_ptr(&self) -> *mut Self {
        self as *const Self as *mut Self
    }
}

#[derive(Default)]
pub struct RBTree<T> {
    root: *mut RBNode<T>,
    size: usize,
}

impl<T> RBTree<T> {
    pub const fn new() -> Self {
        Self {
            root: null_mut(),
            size: 0,
        }
    }
    pub const fn len(&self) -> usize {
        self.size
    }
    pub const fn is_empty(&self) -> bool {
        self.size == 0
    }
}
