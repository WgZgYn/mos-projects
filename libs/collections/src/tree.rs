pub mod rb_tree;
pub mod search_tree;

use core::ptr::null_mut;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum BinaryOrd {
    Left,
    Right,
}

impl core::ops::Not for BinaryOrd {
    type Output = BinaryOrd;

    fn not(self) -> Self::Output {
        match self {
            BinaryOrd::Left => BinaryOrd::Right,
            BinaryOrd::Right => BinaryOrd::Left,
        }
    }
}

impl From<bool> for BinaryOrd {
    fn from(is_left: bool) -> BinaryOrd {
        if is_left {
            BinaryOrd::Left
        } else {
            BinaryOrd::Right
        }
    }
}

pub trait BinaryNode {
    fn left(&self) -> *mut Self;
    fn right(&self) -> *mut Self;
    fn child(&self, is_left: impl Into<BinaryOrd>) -> *mut Self {
        match is_left.into() {
            BinaryOrd::Left => self.left(),
            BinaryOrd::Right => self.right(),
        }
    }
    fn child_mut(&mut self, is_left: impl Into<BinaryOrd>) -> &mut *mut Self {
        match is_left.into() {
            BinaryOrd::Left => self.left_mut(),
            BinaryOrd::Right => self.right_mut(),
        }
    }
    fn left_mut(&mut self) -> &mut *mut Self;
    fn right_mut(&mut self) -> &mut *mut Self;
    fn is_leaf(&self) -> bool {
        self.left().is_null() && self.right().is_null()
    }
    /// This method will change its direct child, return old.
    fn put_child(&mut self, child: *mut Self, is_left: impl Into<BinaryOrd>) -> *mut Self {
        match is_left.into() {
            BinaryOrd::Left => {
                let p = self.left();
                *self.left_mut() = child;
                p
            }
            BinaryOrd::Right => {
                let p = self.right();
                *self.right_mut() = child;
                p
            }
        }
    }
    /// This method will change its direct child, return old.
    fn remove_child(&mut self, is_left: impl Into<BinaryOrd>) -> *mut Self
    where
        Self: Sized,
    {
        self.put_child(null_mut(), is_left)
    }
}

pub trait ParentRefNode {
    fn parent(&self) -> *mut Self;
    fn parent_mut(&mut self) -> &mut *mut Self;
    fn is_root(&self) -> bool {
        self.parent().is_null()
    }
}

pub trait BinaryParentNode: BinaryNode + ParentRefNode {
    fn is_left_child(&self) -> bool {
        unsafe {
            self.parent()
                .as_ref()
                .is_some_and(|parent| core::ptr::eq(parent.left(), self))
        }
    }
    fn child_ord(&self) -> Option<BinaryOrd> {
        if self.parent().is_null() {
            None
        } else {
            Some(self.is_left_child().into())
        }
    }
    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn put_child_with_parent(
        &mut self,
        child: *mut Self,
        is_left: impl Into<BinaryOrd>,
    ) -> *mut Self
    where
        Self: Sized,
    {
        let prev = self.put_child(child, is_left);
        unsafe {
            // set old's parent null
            if let Some(prev) = prev.as_mut() {
                *prev.parent_mut() = null_mut();
            }

            if let Some(node) = child.as_mut() {
                *node.parent_mut() = self;
            }
        }
        prev
    }
    fn remove_child_with_parent(&mut self, is_left: impl Into<BinaryOrd>) -> *mut Self
    where
        Self: Sized,
    {
        self.put_child_with_parent(null_mut(), is_left)
    }

    #[allow(clippy::not_unsafe_ptr_arg_deref)]
    fn inorder_successor(&self) -> *mut Self {
        unsafe {
            let mut prev_ptr = self.parent();
            let mut current_ptr = self.parent();
            while let Some(current_ref) = current_ptr.as_ref() {
                prev_ptr = current_ptr;
                current_ptr = current_ref.left();
            }
            prev_ptr
        }
    }
}
impl<T> BinaryParentNode for T where T: BinaryNode + ParentRefNode {}

fn rotate<Node>(root_ptr_ref: &mut *mut Node, node: *mut Node, ord: impl Into<BinaryOrd>)
where
    Node: BinaryParentNode + Sized,
{
    let ord = ord.into();
    unsafe {
        let Some(node_ref) = node.as_mut() else {
            return;
        };
        let Some(child_ref) = node_ref.child(!ord).as_mut() else {
            return;
        };
        let is_left_node = node_ref.is_left_child();
        let child_child = child_ref.child(ord);
        let parent = node_ref.parent();

        node_ref.put_child_with_parent(child_child, !ord);
        child_ref.put_child_with_parent(node_ref, ord);

        if let Some(parent) = parent.as_mut() {
            parent.put_child_with_parent(child_ref, is_left_node);
        } else {
            *root_ptr_ref = child_ref;
        }
    }
}

pub fn replace_node_with_parent<Node: BinaryParentNode>(
    root: &mut *mut Node,
    old_ref: &mut Node,
    new_ref: &mut Node,
) {
    let parent = old_ref.parent();
    let left = old_ref.left();
    let right = old_ref.right();
    let is_left = old_ref.is_left_child();

    // update child links
    new_ref.put_child_with_parent(left, BinaryOrd::Left);
    new_ref.put_child_with_parent(right, BinaryOrd::Right);

    // update parent link
    unsafe {
        let Some(parent_ref) = parent.as_mut() else {
            *root = new_ref;
            return;
        };
        parent_ref.put_child_with_parent(new_ref, is_left);
    }

    // clear old links
    *old_ref.parent_mut() = null_mut();
    *old_ref.left_mut() = null_mut();
    *old_ref.right_mut() = null_mut();
}

pub fn replace_node<Node: BinaryNode>(
    root: &mut *mut Node,
    old_ref: &mut Node,
    new_ref: &mut Node,
) {
    let left = old_ref.left();
    let right = old_ref.right();

    // update child links
    new_ref.put_child(left, BinaryOrd::Left);
    new_ref.put_child(right, BinaryOrd::Right);

    // clear old links
    *old_ref.left_mut() = null_mut();
    *old_ref.right_mut() = null_mut();

    if *root == old_ref {
        *root = new_ref;
    }
}
