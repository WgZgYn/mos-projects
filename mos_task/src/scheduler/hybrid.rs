use crate::scheduler::idle_task;
use crate::thread::ThreadControlBlock;
use crate::traits::{ReadyQueue, SchedDecision, SchedStrategy};
use core::marker::PhantomData;
use mos_lib::sync::KRef;

pub struct HybridSchedStrategy<S>(PhantomData<S>);

pub struct StrategyQueue<S: SchedStrategy> {
    queue: S::Queue,
}

impl<S: SchedStrategy> StrategyQueue<S> {
    fn new() -> Self {
        Self {
            queue: S::Queue::new(),
        }
    }
}

pub struct HybridSchedReadyQueue<S: SchedStrategy> {
    tasks: StrategyQueue<S>,
    idle: KRef<ThreadControlBlock>,
}

impl<S: SchedStrategy> ReadyQueue for HybridSchedReadyQueue<S> {
    fn push(&mut self, t: KRef<ThreadControlBlock>) {
        if t.tid != self.idle.tid {
            self.tasks.queue.push(t);
        }
    }

    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.tasks.queue.pop().or_else(|| Some(self.idle.clone()))
    }

    fn peek(&self) -> Option<&KRef<ThreadControlBlock>> {
        self.tasks.queue.peek().or(Some(&self.idle))
    }

    fn new() -> Self {
        Self {
            tasks: StrategyQueue::new(),
            idle: idle_task(),
        }
    }
}

impl<S: SchedStrategy> SchedStrategy for HybridSchedStrategy<S> {
    type Queue = HybridSchedReadyQueue<S>;
    fn pick_with(queue: &mut Self::Queue, task: KRef<ThreadControlBlock>) -> SchedDecision {
        if task.tid == queue.idle.tid {
            let Some(next) = S::pick_next(&mut queue.tasks.queue) else {
                return SchedDecision::KeepRunning;
            };
            SchedDecision::SwitchTo(next)
        } else {
            S::pick_with(&mut queue.tasks.queue, task)
        }
    }
}
