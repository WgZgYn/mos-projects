use crate::thread::ThreadControlBlock;
use mos_lib::sync::KRef;

pub trait ReadyQueue {
    fn push(&mut self, t: KRef<ThreadControlBlock>);
    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>>;
    fn push_pop(&mut self, t: KRef<ThreadControlBlock>) -> KRef<ThreadControlBlock> {
        self.push(t);
        self.pop().unwrap()
    }
    fn peek(&self) -> Option<&KRef<ThreadControlBlock>>;
    fn new() -> Self;
}

pub trait Scheduler {
    /// enqueue task
    fn enqueue(&mut self, task: KRef<ThreadControlBlock>);

    /// this method will take the `current` from PerCPU and finally execute context_switch
    fn schedule(&mut self, current: &mut KRef<ThreadControlBlock>) -> KRef<ThreadControlBlock>;
}

pub enum SchedDecision {
    KeepRunning,
    SwitchTo(KRef<ThreadControlBlock>),
}

pub trait SchedStrategy {
    type Queue: ReadyQueue;

    fn pick_next(queue: &mut Self::Queue) -> Option<KRef<ThreadControlBlock>> {
        queue.pop()
    }

    fn pick_with(queue: &mut Self::Queue, task: KRef<ThreadControlBlock>) -> SchedDecision {
        let tid = task.tid;
        let next = queue.push_pop(task);
        if next.tid == tid {
            SchedDecision::KeepRunning
        } else {
            SchedDecision::SwitchTo(next)
        }
    }
}
