use crate::thread::ThreadControlBlock;
use alloc::collections::VecDeque;
use core::ops::{Deref, DerefMut};
use mos_lib::sync::KRef;

pub struct WaitQueueInner(VecDeque<KRef<ThreadControlBlock>>);

impl Default for WaitQueueInner {
    fn default() -> Self {
        Self::new()
    }
}

impl WaitQueueInner {
    pub const fn new() -> Self {
        WaitQueueInner(VecDeque::new())
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn wait_task(&mut self, task: KRef<ThreadControlBlock>) {
        self.0.push_back(task);
    }
    pub fn wake_one(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.0.pop_front()
    }
    pub fn wake_all(&mut self) -> impl IntoIterator<Item = KRef<ThreadControlBlock>> {
        self.0.drain(..)
    }
}

/// 等待队列 — 使用 spin::Mutex 而非 SpinIrqLock。
/// 所有 WaitQueue 访问都在 SCHEDULER 锁内 (已关中断),
/// 若用 SpinIrqLock 会在 Park 路径上恢复 IRQ 导致同核死锁。
pub struct WaitQueue {
    queue: spin::Mutex<WaitQueueInner>,
}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
    }
}

pub struct WaitQueueGuard<'a> {
    guard: spin::MutexGuard<'a, WaitQueueInner>,
}

impl WaitQueue {
    pub const fn new() -> Self {
        Self {
            queue: spin::Mutex::new(WaitQueueInner(VecDeque::new())),
        }
    }
    pub fn lock(&'_ self) -> WaitQueueGuard<'_> {
        WaitQueueGuard {
            guard: self.queue.lock(),
        }
    }
}

impl<'a> Deref for WaitQueueGuard<'a> {
    type Target = spin::MutexGuard<'a, WaitQueueInner>;

    fn deref(&self) -> &Self::Target {
        &self.guard
    }
}

impl<'a> DerefMut for WaitQueueGuard<'a> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.guard
    }
}
