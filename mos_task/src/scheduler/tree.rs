use crate::thread::ThreadControlBlock;
use crate::traits::{ReadyQueue, SchedStrategy};
use alloc::collections::BTreeSet;
use core::cmp::Ordering;
use core::ops::Deref;
use mos_lib::sync::KRef;

pub struct TreeScheduleStrategy;

pub struct TreeEntry(KRef<ThreadControlBlock>);

impl Eq for TreeEntry {}

impl PartialEq<Self> for TreeEntry {
    fn eq(&self, other: &Self) -> bool {
        self.0.cfs_key() == other.0.cfs_key()
    }
}

impl PartialOrd<Self> for TreeEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TreeEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cfs_key().cmp(&(other.0.cfs_key()))
    }
}

pub struct TreeQueue {
    tasks: BTreeSet<TreeEntry>,
}

impl ReadyQueue for TreeQueue {
    fn push(&mut self, t: KRef<ThreadControlBlock>) {
        self.tasks.insert(TreeEntry(t));
    }

    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.tasks.pop_first().map(|t| t.0)
    }

    fn peek(&self) -> Option<&ThreadControlBlock> {
        self.tasks.first().map(|t| t.0.deref())
    }

    fn new() -> Self {
        Self {
            tasks: BTreeSet::new(),
        }
    }
}

impl SchedStrategy for TreeScheduleStrategy {
    type Queue = TreeQueue;
}
