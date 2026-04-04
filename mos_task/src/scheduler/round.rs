#![allow(unused)]

use crate::scheduler::Dequeue;
use crate::thread::ThreadControlBlock;
use crate::traits::{ReadyQueue, SchedStrategy};
use alloc::collections::VecDeque;
use core::ops::Deref;
use mos_lib::sync::KRef;

pub struct RoundScheduleStrategy;

impl SchedStrategy for RoundScheduleStrategy {
    type Queue = Dequeue<KRef<ThreadControlBlock>>;
}
