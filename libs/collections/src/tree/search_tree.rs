use crate::tree::{
    BinaryNode, BinaryOrd, BinaryParentNode, replace_node, replace_node_with_parent,
};
use core::cmp::Ordering;

/// return Some when the old node has the same key with new node, return old node
pub fn insert_search_tree<Node: BinaryNode + Ord>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
) -> Option<*mut Node> {
    insert_search_tree_by(root_ptr_ref, node, Ord::cmp)
}

/// return Some when the old node has the same key with new node, return old node
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn insert_search_tree_by<Node, F>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
    cmp: F,
) -> Option<*mut Node>
where
    Node: BinaryNode,
    F: Fn(&Node, &Node) -> Ordering,
{
    unsafe {
        let node_ref = node.as_mut()?;
        let Some(root_ref) = root_ptr_ref.as_mut() else {
            *root_ptr_ref = node;
            return None;
        };

        let mut current_ref = root_ref;
        loop {
            match cmp(node_ref, current_ref) {
                Ordering::Less => {
                    if let Some(left_node_ref) = current_ref.left().as_mut() {
                        current_ref = left_node_ref;
                    } else {
                        current_ref.put_child(node_ref, BinaryOrd::Left);
                        return None;
                    }
                }
                Ordering::Equal => {
                    replace_node(root_ptr_ref, current_ref, node_ref);
                    return Some(current_ref);
                }
                Ordering::Greater => {
                    if let Some(right_node_ref) = current_ref.right().as_mut() {
                        current_ref = right_node_ref;
                    } else {
                        current_ref.put_child(node_ref, BinaryOrd::Right);
                        return None;
                    }
                }
            }
        }
    }
}

pub fn insert_search_tree_with_parent<Node: BinaryParentNode + Ord>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
) -> Option<*mut Node> {
    insert_search_tree_with_parent_by(root_ptr_ref, node, Ord::cmp)
}

/// return Some when the old node has the same key with new node, return old node
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn insert_search_tree_with_parent_by<Node, F>(
    root_ptr_ref: &mut *mut Node,
    node: *mut Node,
    cmp: F,
) -> Option<*mut Node>
where
    Node: BinaryParentNode,
    F: Fn(&Node, &Node) -> Ordering,
{
    unsafe {
        let node_ref = node.as_mut()?;
        let Some(root_ref) = root_ptr_ref.as_mut() else {
            *root_ptr_ref = node;
            return None;
        };

        let mut current_ref = root_ref;
        loop {
            match cmp(node_ref, current_ref) {
                Ordering::Less => {
                    if let Some(left_node_ref) = current_ref.left().as_mut() {
                        current_ref = left_node_ref;
                    } else {
                        current_ref.put_child_with_parent(node_ref, BinaryOrd::Left);
                        return None;
                    }
                }
                Ordering::Equal => {
                    replace_node_with_parent(root_ptr_ref, current_ref, node_ref);
                    return Some(current_ref);
                }
                Ordering::Greater => {
                    if let Some(right_node_ref) = current_ref.right().as_mut() {
                        current_ref = right_node_ref;
                    } else {
                        current_ref.put_child_with_parent(node_ref, BinaryOrd::Right);
                        return None;
                    }
                }
            }
        }
    }
}
