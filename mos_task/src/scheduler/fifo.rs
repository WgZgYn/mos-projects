#![allow(unused)]

use crate::scheduler::Dequeue;
use crate::thread::ThreadControlBlock;
use crate::traits::SchedStrategy;
use mos_lib::sync::KRef;

pub struct FifoScheduleStrategy;
impl SchedStrategy for FifoScheduleStrategy {
    type Queue = Dequeue<KRef<ThreadControlBlock>>;
}
