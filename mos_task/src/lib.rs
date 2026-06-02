#![no_std]
extern crate alloc;

use crate::thread::ThreadControlBlock;
use mos_lib::sync::KRef;

mod block_mutex;
pub mod preempt_session;
pub mod process;
pub mod scheduler;
pub mod thread;
mod traits;
pub mod wait_queue;
pub mod semaphore;

/// enqueue ready task
pub fn enqueue_task(task: KRef<ThreadControlBlock>) {
    scheduler::SCHEDULER.lock().enqueue_task(task);
}

pub use scheduler::{current_tcb, first_schedule, schedule, scheduler_debug_info};
