#![allow(unused)]

use crate::scheduler::{Dequeue, idle_task};
use crate::thread::ThreadControlBlock;
use crate::traits::{ReadyQueue, SchedDecision, SchedStrategy};
use mos_lib::sync::KRef;

pub struct FcfsScheduleStrategy;
impl SchedStrategy for FcfsScheduleStrategy {
    type Queue = Dequeue<KRef<ThreadControlBlock>>;

    fn pick_with(queue: &mut Self::Queue, task: KRef<ThreadControlBlock>) -> SchedDecision {
        if task.tid == idle_task().tid
            && let Some(next) = queue.pop()
        {
            queue.push(task);
            return SchedDecision::SwitchTo(next);
        }

        SchedDecision::KeepRunning
    }
}
