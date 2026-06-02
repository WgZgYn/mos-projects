use crate::scheduler::{ParkDecision, park_current, wake_blocked_task};
use crate::thread::ThreadControlBlock;
use crate::wait_queue::WaitQueueInner;
use alloc::vec::Vec;
use mos_lib::sync::KRef;
use spin::Mutex;

#[derive(Default)]
struct Semaphore {
    count: usize,
    queue: WaitQueueInner,
}

impl Semaphore {
    fn new(init: usize) -> Self {
        Self {
            count: init,
            queue: WaitQueueInner::new(),
        }
    }
    fn signal(&mut self) -> Option<KRef<ThreadControlBlock>> {
        if self.queue.is_empty() {
            self.count += 1;
            None
        } else {
            self.queue.wake_one()
        }
    }
}

pub struct SemaphoreTable {
    semaphores: mos_mem::slab::Slab<Semaphore>,
}

impl SemaphoreTable {
    fn singleton() -> &'static Mutex<Self> {
        static DATA: Mutex<SemaphoreTable> = Mutex::new(SemaphoreTable {
            semaphores: mos_mem::slab::Slab::new(),
        });
        &DATA
    }

    pub fn create(init: usize) -> usize {
        Self::singleton().lock().semaphores.insert(Semaphore::new(init))
    }

    pub fn signal(index: usize) {
        let mut table = Self::singleton().lock();
        if let Some(sem) = table.semaphores.get_mut(index) {
            let task = sem.signal();
            drop(table); // 先释放 table 锁, 再唤醒
            wake_blocked_task(task);
        }
    }

    pub fn wait(index: usize) {
        let mut table = Self::singleton().lock();

        let sem = match table.semaphores.get_mut(index) {
            Some(s) => s,
            None => return,
        };

        if sem.count > 0 {
            sem.count -= 1;
            return;
        }

        park_current(|task| {
            match table.semaphores.get_mut(index) {
                Some(sem) => {
                    if sem.count > 0 {
                        sem.count -= 1;
                        ParkDecision::Unpark
                    } else {
                        sem.queue.wait_task(task.clone());
                        ParkDecision::Park(table)
                    }
                }
                None => ParkDecision::Unpark,
            }
        });
        // Park'd 被 signal 唤醒 → 资源已转移, 获取成功
        // Unpark'd → count 已被 decrement, 获取成功
        // 无论哪种情况, park_current 返回 = 已获取信号量, 退出
    }

    pub fn destroy(index: usize) -> bool {
        let mut table = Self::singleton().lock();
        if table.semaphores.get(index).is_some() {
            // 唤醒所有等待者 (它们的 wait() loop 会发现 sem 不存在而退出)
            let tasks: Vec<_> = table
                .semaphores
                .get_mut(index)
                .into_iter()
                .flat_map(|sem| sem.queue.wake_all())
                .collect();
            table.semaphores.remove(index);
            drop(table);
            wake_blocked_task(tasks);
            true
        } else {
            false
        }
    }
}
