use crate::thread::{Priority, ThreadControlBlock};
use crate::traits::{ReadyQueue, SchedDecision, SchedStrategy};
use alloc::collections::{BinaryHeap, VecDeque};
use core::cmp::Ordering;
use core::fmt::{Debug, Formatter};
use mos_lib::sync::KRef;

pub const DEFAULT_CAPACITY: usize = 32;

#[derive(Debug)]
pub struct PriorityQueue<T>(BinaryHeap<T>);

#[derive(Clone)]
pub struct PriorityEntry(KRef<ThreadControlBlock>);

impl Debug for PriorityEntry {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.write_fmt(format_args!("{:?}", self.0.tid))
    }
}

impl PriorityEntry {
    const fn new(thread: KRef<ThreadControlBlock>) -> Self {
        PriorityEntry(thread)
    }
    fn priority(&self) -> Priority {
        self.0.priority
    }
}

impl Eq for PriorityEntry {}

impl PartialEq<Self> for PriorityEntry {
    fn eq(&self, other: &Self) -> bool {
        self.priority() == other.priority()
    }
}

impl PartialOrd<Self> for PriorityEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PriorityEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        self.priority().cmp(&other.priority())
    }
}

impl ReadyQueue for PriorityQueue<PriorityEntry> {
    fn push(&mut self, t: KRef<ThreadControlBlock>) {
        self.0.push(PriorityEntry(t));
    }
    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.0.pop().map(|entry| entry.0)
    }
    fn peek(&self) -> Option<&KRef<ThreadControlBlock>> {
        self.0.peek().map(|entry| &entry.0)
    }
    fn new() -> Self {
        PriorityQueue(BinaryHeap::with_capacity(DEFAULT_CAPACITY))
    }
}

impl<T: Ord> PriorityQueue<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        PriorityQueue(BinaryHeap::with_capacity(capacity))
    }
}

pub struct PriorityScheduleStrategy;

impl SchedStrategy for PriorityScheduleStrategy {
    type Queue = PriorityQueue<PriorityEntry>;

    fn pick_with(queue: &mut Self::Queue, task: KRef<ThreadControlBlock>) -> SchedDecision {
        if let Some(next) = queue.peek()
            && next.priority >= task.priority
        {
            SchedDecision::SwitchTo(queue.push_pop(task))
        } else {
            SchedDecision::KeepRunning
        }
    }
}

struct StaticMultiLevelPriorityQueue<const N: usize> {
    queues: [VecDeque<KRef<ThreadControlBlock>>; N],
}

impl<const N: usize> ReadyQueue for StaticMultiLevelPriorityQueue<N> {
    fn push(&mut self, _t: KRef<ThreadControlBlock>) {
        todo!()
    }

    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        todo!()
    }

    fn push_pop(&mut self, _t: KRef<ThreadControlBlock>) -> KRef<ThreadControlBlock> {
        todo!()
    }

    fn peek(&self) -> Option<&KRef<ThreadControlBlock>> {
        todo!()
    }
    fn new() -> Self {
        todo!()
    }
}
