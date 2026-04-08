use crate::thread::{ThreadControlBlock, Tid};
use crate::traits::{ReadyQueue, SchedStrategy};
use core::ptr::NonNull;
use intrusive_collections::{
    KeyAdapter, PointerOps, RBTree, RBTreeLink, UnsafeRef, intrusive_adapter,
};
use log::trace;
use mos_lib::sync::KRef;

pub struct CfsScheduleStrategy;

intrusive_adapter!(RBAdapter = UnsafeRef<ThreadControlBlock>: ThreadControlBlock { node => RBTreeLink });

impl<'a> KeyAdapter<'a> for RBAdapter {
    type Key = (u64, Tid);

    fn get_key(&self, value: &'a <Self::PointerOps as PointerOps>::Value) -> Self::Key {
        value.cfs_key()
    }
}

pub struct CfsReadyQueue {
    root: RBTree<RBAdapter>,
    min: Option<UnsafeRef<ThreadControlBlock>>, // min is ref of self.root
}

impl ReadyQueue for CfsReadyQueue {
    fn push(&mut self, t: KRef<ThreadControlBlock>) {
        unsafe {
            let key = t.cfs_key();
            let t = KRef::into_ptr(t);
            let task_ref = UnsafeRef::from_raw(t.as_ptr());
            self.root.insert(task_ref.clone());

            // cache
            if let Some(min) = &self.min {
                let min_key = min.cfs_key();
                if key < min_key {
                    self.min = Some(task_ref);
                    trace!(
                        "update cfs min cache from {}:{:?} to {}:{:?}",
                        min_key.0, min_key.1, key.0, key.1
                    );
                }
            } else {
                self.min = Some(task_ref);
            }
        }
    }

    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        let old_min = self.min.as_ref()?;
        let mut cursor = unsafe { self.root.cursor_mut_from_ptr(old_min.as_ref()) };
        let old = cursor.remove().expect("Some(min) but cursor is empty");
        unsafe {
            self.min = cursor.get().map(|t| UnsafeRef::from_raw(t));
            Some(KRef::from_ptr(NonNull::new_unchecked(UnsafeRef::into_raw(
                old,
            ))))
        }
    }

    /// because if the t has higher weight, will pop t.
    /// so this method never will cache min with t.
    fn push_pop(&mut self, t: KRef<ThreadControlBlock>) -> KRef<ThreadControlBlock> {
        let Some(min) = self.min.as_ref() else {
            return t;
        };

        let key = t.cfs_key();
        let min_key = min.cfs_key();

        if key < min_key {
            return t;
        }

        unsafe {
            let t = KRef::into_ptr(t);
            self.root.insert(UnsafeRef::from_raw(t.as_ptr()));
            let mut cursor = self.root.cursor_mut_from_ptr(min.as_ref());
            let min_arc = cursor.remove().expect("Some(min) but cursor is empty");
            self.min = cursor.get().map(|t| UnsafeRef::from_raw(t));
            KRef::from_ptr(NonNull::new_unchecked(UnsafeRef::into_raw(min_arc)))
        }
    }

    fn peek(&self) -> Option<&ThreadControlBlock> {
        let min = self.min.as_ref()?;
        let cursor = unsafe { self.root.cursor_from_ptr(min.as_ref()) };
        cursor.get()
    }

    fn new() -> Self {
        Self {
            root: RBTree::new(RBAdapter::new()),
            min: None,
        }
    }
}

impl SchedStrategy for CfsScheduleStrategy {
    type Queue = CfsReadyQueue;
}
