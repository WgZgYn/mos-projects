#![no_std]
extern crate alloc;

use crate::thread::{ThreadControlBlock, ThreadStateValue};
use mos_lib::sync::KRef;

pub mod block_mutex;
pub mod process;
mod scheduler;
pub mod thread;
pub mod traits;
pub mod wait_queue;

/// enqueue ready task
pub fn enqueue_task(task: KRef<ThreadControlBlock>) {
    assert_eq!(task.get_state(), ThreadStateValue::Ready);
    SCHEDULER.access(|scheduler| {
        scheduler.lock().enqueue_task(task);
    })
}

pub use scheduler::SCHEDULER;
pub use scheduler::{
    CfsScheduleStrategy, FifoScheduleStrategy, PriorityScheduleStrategy, RoundScheduleStrategy,
    TreeScheduleStrategy,
};
pub use scheduler::{current_tcb, first_schedule, schedule, scheduler_debug_info};
