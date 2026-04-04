use crate::thread::{ThreadControlBlock, ThreadStateValue};
use crate::traits::Waitable;
use alloc::collections::VecDeque;
use core::ops::{Deref, DerefMut};
use mos_arch::sync::{SpinIrqLock, SpinIrqLockGuard};
use mos_lib::sync::{KRef, KWake};

pub struct WaitQueueInner(VecDeque<KRef<ThreadControlBlock>>);

impl WaitQueueInner {
    pub fn enqueue(&mut self, block: KRef<ThreadControlBlock>) {
        self.0.push_back(block);
    }
    pub fn dequeue(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.0.pop_front()
    }
}

pub struct WaitQueue {
    queue: SpinIrqLock<WaitQueueInner>,
}

pub struct WaitQueueGuard<'a> {
    guard: SpinIrqLockGuard<'a, WaitQueueInner>,
}

impl<'a> Deref for WaitQueueGuard<'a> {
    type Target = SpinIrqLockGuard<'a, WaitQueueInner>;
    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a> DerefMut for WaitQueueGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            queue: SpinIrqLock::new(WaitQueueInner(VecDeque::new())),
        }
    }
    pub fn wait(&self, tcb: KRef<ThreadControlBlock>) {
        tcb.set_state(ThreadStateValue::Blocked);
        self.queue.access(|q| q.enqueue(tcb));
    }
    pub fn wake_one(&self) -> bool {
        let tcb = self.queue.access(|q| q.dequeue());
        if let Some(tcb) = tcb {
            KWake::wake(tcb);
            true
        } else {
            false
        }
    }
    pub fn wake_all(&self) -> usize {
        let mut n = 0;
        self.queue.access(|q| {
            while let Some(tcb) = q.dequeue() {
                KWake::wake(tcb);
                n += 1;
            }
        });
        n
    }
    pub fn lock(&self) -> WaitQueueGuard<'_> {
        WaitQueueGuard {
            guard: self.queue.lock(),
        }
    }
    pub fn has_waiters(&self) -> bool {
        let guard = self.queue.lock();
        guard.0.len() > 0
    }
}

impl Waitable for WaitQueue {
    fn wait(&self, current: KRef<ThreadControlBlock>) {
        self.wait(current);
    }
    fn wake_one(&self) -> bool {
        self.wake_one()
    }
    fn wake_all(&self) -> usize {
        self.wake_all()
    }
}
