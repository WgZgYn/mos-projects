use crate::tree::search_tree::{
    insert_search_tree_with_parent, insert_search_tree_with_parent_by,
};
use crate::tree::{replace_node_with_parent, rotate, BinaryNode, BinaryParentNode, ParentRefNode};
use core::cmp::Ordering;
use core::ptr::null_mut;

pub mod alloc;
pub mod intrusive;
pub mod macros;

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum RBNodeColor {
    Red,
    Black,
}
impl RBNodeColor {
    pub fn reverse(&mut self) {
        match self {
            RBNodeColor::Red => *self = RBNodeColor::Black,
            RBNodeColor::Black => *self = RBNodeColor::Red,
        }
    }
}

impl core::ops::Not for RBNodeColor {
    type Output = RBNodeColor;
    fn not(mut self) -> RBNodeColor {
        self.reverse();
        self
    }
}

pub trait RedBlackNode: BinaryNode + ParentRefNode {
    fn color(&self) -> RBNodeColor;
    fn color_mut(&mut self) -> &mut RBNodeColor;
}

fn rb_insert_fixup<Node: RedBlackNode>(root_ptr_ref: &mut *mut Node, node: *mut Node) {
    fn uncle<Node: ParentRefNode + BinaryNode>(grand: &Node, parent: *mut Node) -> *mut Node {
        if grand.left() == parent {
            grand.right()
        } else {
            grand.left()
        }
    }
    unsafe {
        let Some(node_ref) = node.as_mut() else {
            return;
        };
        if core::ptr::eq(*root_ptr_ref, node_ref) {
            *node_ref.color_mut() = RBNodeColor::Black;
            return;
        }
        *node_ref.color_mut() = RBNodeColor::Red;
        let parent_ref = node_ref
            .parent()
            .as_mut()
            .expect("should have parent if not root");
        if parent_ref.color() == RBNodeColor::Black {
            return;
        }
        let grand_ref = parent_ref
            .parent()
            .as_mut()
            .expect("should have parent if not BLACK");
        let parent_to_child = parent_ref.left() == node_ref;
        let grand_to_parent = grand_ref.left() == parent_ref;
        let uncle = uncle(grand_ref, node_ref);
        match uncle
            .as_ref()
            .map(|n| n.color())
            .unwrap_or(RBNodeColor::Black)
        {
            RBNodeColor::Black => {
                if grand_to_parent == parent_to_child {
                    // LL | RR
                    rotate(root_ptr_ref, grand_ref, !grand_to_parent);
                    parent_ref.color_mut().reverse();
                    grand_ref.color_mut().reverse();
                } else {
                    // LR | RL
                    rotate(root_ptr_ref, parent_ref, grand_to_parent);
                    rotate(root_ptr_ref, grand_ref, parent_to_child);
                    node_ref.color_mut().reverse();
                    grand_ref.color_mut().reverse();
                }
            }
            RBNodeColor::Red => {
                if let Some(n) = uncle.as_mut() {
                    n.color_mut().reverse()
                }
                parent_ref.color_mut().reverse();
                grand_ref.color_mut().reverse();
                rb_insert_fixup(root_ptr_ref, grand_ref);
            }
        }
    }
}

/// this method take the mut ref of the root.
/// if the root is null, the root will be replaced by node.
/// if there having two same key, it will replace the old node with new node.
/// if the node is_some and all the ptr ops is legal, return `None` means size += 1
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn rb_insert_by<Node, F>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
    cmp: F,
) -> Option<*mut Node>
where
    Node: RedBlackNode,
    F: Fn(&Node, &Node) -> Ordering,
{
    let replaced = insert_search_tree_with_parent_by(root_ptr_ref, node, cmp);
    rb_insert_fixup(root_ptr_ref, node);
    replaced
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn rb_insert_by_key<Node, F, V>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
    key: F,
) -> Option<*mut Node>
where
    Node: RedBlackNode,
    F: Fn(&Node) -> V,
    V: Ord,
{
    let replaced =
        insert_search_tree_with_parent_by(root_ptr_ref, node, |a, b| key(a).cmp(&key(b)));
    rb_insert_fixup(root_ptr_ref, node);
    replaced
}

pub fn rb_insert<Node>(root_ptr_ref: &mut *mut Node, node: *mut Node) -> Option<*mut Node>
where
    Node: RedBlackNode + Ord,
{
    let replaced = insert_search_tree_with_parent(root_ptr_ref, node);
    rb_insert_fixup(root_ptr_ref, node);
    replaced
}

#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn rb_remove_node<Node: RedBlackNode>(
    root_ptr_ref: &mut *mut Node,
    node_ref: &mut Node,
) -> *mut Node {
    unsafe {
        let target = node_ref as *mut Node;

        // ===== 1. 若有两个子节点 =====
        if !node_ref.left().is_null() && !node_ref.right().is_null() {
            let succ = node_ref.inorder_successor();
            let succ_ref = succ.as_mut().unwrap();

            // successor 原来的 child（只能是右孩子）
            let child = succ_ref.right();
            let parent = succ_ref.parent();
            let is_left = succ_ref.is_left_child();
            let removed_color = succ_ref.color();

            // 先从原位置删除 successor
            if let Some(parent_ref) = parent.as_mut() {
                parent_ref.put_child_with_parent(child, is_left);
            } else {
                *root_ptr_ref = child;
                if let Some(child_ref) = child.as_mut() {
                    *child_ref.parent_mut() = null_mut();
                }
            }

            // successor 替换 node
            replace_node_with_parent(root_ptr_ref, node_ref, succ_ref);

            // 修复
            if removed_color == RBNodeColor::Black {
                rb_delete_fixup(root_ptr_ref, parent, child, is_left);
            }

            return succ;
        }

        // ===== 2. 至多一个 child =====
        let target_ref = target.as_mut().unwrap();

        let child = if !target_ref.left().is_null() {
            target_ref.left()
        } else {
            target_ref.right()
        };

        let parent = target_ref.parent();
        let is_left = target_ref.is_left_child();
        let removed_color = target_ref.color();

        // ===== 3. replace target =====
        if let Some(parent_ref) = parent.as_mut() {
            parent_ref.put_child_with_parent(child, is_left);
        } else {
            *root_ptr_ref = child;
            if let Some(child_ref) = child.as_mut() {
                *child_ref.parent_mut() = null_mut();
            }
        }

        // clear links
        *target_ref.parent_mut() = null_mut();
        target_ref.put_child(null_mut(), true);
        target_ref.put_child(null_mut(), false);

        // ===== 4. fixup =====
        if removed_color == RBNodeColor::Black {
            rb_delete_fixup(root_ptr_ref, parent, child, is_left);
        }

        target
    }
}

fn rb_delete_fixup<Node: RedBlackNode>(
    root_ptr_ref: &mut *mut Node,
    mut parent: *mut Node,
    mut node: *mut Node,
    mut is_left: bool,
) {
    unsafe {
        while !core::ptr::eq(node, *root_ptr_ref)
            && node
                .as_ref()
                .map(|n| n.color() == RBNodeColor::Black)
                .unwrap_or(true)
        {
            let parent_ref = match parent.as_mut() {
                Some(p) => p,
                None => break,
            };

            let mut sibling = parent_ref.child(!is_left);
            // ===== Case 1: sibling red =====
            if sibling
                .as_ref()
                .map(|s| s.color() == RBNodeColor::Red)
                .unwrap_or(false)
            {
                sibling.as_mut().unwrap().color_mut().reverse();
                parent_ref.color_mut().reverse();

                rotate(root_ptr_ref, parent, is_left);

                sibling = if is_left {
                    parent_ref.right()
                } else {
                    parent_ref.left()
                };
            }

            let sibling_ref = sibling.as_mut().unwrap();

            let left_black = sibling_ref
                .left()
                .as_ref()
                .map(|n| n.color() == RBNodeColor::Black)
                .unwrap_or(true);

            let right_black = sibling_ref
                .right()
                .as_ref()
                .map(|n| n.color() == RBNodeColor::Black)
                .unwrap_or(true);

            // ===== Case 2 =====
            if left_black && right_black {
                sibling_ref.color_mut().reverse();
                node = parent;
                parent = parent_ref.parent();
                is_left = parent_ref.is_left_child();
            } else {
                // ===== Case 3 =====
                if is_left && right_black {
                    sibling_ref.left().as_mut().unwrap().color_mut().reverse();
                    sibling_ref.color_mut().reverse();
                    rotate(root_ptr_ref, sibling, false);
                    sibling = parent_ref.right();
                } else if !is_left && left_black {
                    sibling_ref.right().as_mut().unwrap().color_mut().reverse();
                    sibling_ref.color_mut().reverse();
                    rotate(root_ptr_ref, sibling, true);
                    sibling = parent_ref.left();
                }

                // ===== Case 4 =====
                let sibling_ref = sibling.as_mut().unwrap();
                *sibling_ref.color_mut() = parent_ref.color();
                *parent_ref.color_mut() = RBNodeColor::Black;

                if is_left {
                    sibling_ref.right().as_mut().unwrap().color_mut().reverse();
                } else {
                    sibling_ref.left().as_mut().unwrap().color_mut().reverse();
                }

                rotate(root_ptr_ref, parent, is_left);

                node = *root_ptr_ref;
                break;
            }
        }

        if let Some(node_ref) = node.as_mut() {
            *node_ref.color_mut() = RBNodeColor::Black;
        }
    }
}
