use crate::tree::rb_tree::RedBlackNode;
use crate::tree::ParentRefNode;
use crate::tree::BinaryNode;
use crate::{impl_binary_node, impl_parent_node, impl_rb_node};
use core::ptr::null_mut;
use crate::tree::BinaryParentNode;
use crate::tree::rb_tree::RBNodeColor;

pub struct RBNode {
    parent: *mut RBNode,
    left: *mut RBNode,
    right: *mut RBNode,
    color: RBNodeColor,
}

impl_binary_node!(RBNode);
impl_parent_node!(RBNode);
impl_rb_node!(RBNode);

impl RBNode {
    pub const fn default() -> Self {
        Self::new(RBNodeColor::Red)
    }
    pub const fn new(color: RBNodeColor) -> Self {
        Self {
            parent: null_mut(),
            left: null_mut(),
            right: null_mut(),
            color,
        }
    }
    pub const fn as_ptr(&self) -> *mut Self {
        self as *const Self as *mut Self
    }
}

#[derive(Default)]
pub struct RBTree {
    root: *mut RBNode,
    size: usize,
}

impl RBTree {
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
        self.len() == 0
    }
}