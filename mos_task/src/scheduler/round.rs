#![allow(unused)]

use crate::scheduler::Dequeue;
use crate::thread::ThreadControlBlock;
use crate::traits::{ReadyQueue, SchedDecision, SchedStrategy};
use alloc::collections::VecDeque;
use core::ops::Deref;
use core::sync::atomic::{AtomicUsize, Ordering};
use mos_lib::sync::KRef;

pub struct RoundScheduleStrategy;

static TICKS: AtomicUsize = AtomicUsize::new(0);

impl SchedStrategy for RoundScheduleStrategy {
    type Queue = Dequeue<KRef<ThreadControlBlock>>;

    fn pick_with(queue: &mut Self::Queue, task: KRef<ThreadControlBlock>) -> SchedDecision {
        if TICKS.load(Ordering::Relaxed).is_multiple_of(10) {
            SchedDecision::SwitchTo(queue.push_pop(task))
        } else {
            SchedDecision::KeepRunning
        }
    }
}
