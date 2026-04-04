mod cfs;
mod fifo;
mod priority;
mod round;
mod tree;

use crate::thread::{IDLE, REAPER, ThreadControlBlock, ThreadStateValue};
use crate::traits::{ReadyQueue, SchedStrategy};
use alloc::collections::VecDeque;
use core::arch::asm;
use core::marker::PhantomData;
use core::ops::Deref;
use lazy_static::lazy_static;
use log::info;
use mos_arch::asm::__switch_context;
use mos_arch::gdt::write_rsp0;
use mos_arch::percpu::update_per_cpu;
use mos_arch::sync::IrqLocked;
use mos_lib::sync::KRef;
use x86_64::structures::idt::InterruptStackFrame;

pub use cfs::CfsScheduleStrategy;
pub use fifo::FifoScheduleStrategy;
pub use priority::PriorityScheduleStrategy;
pub use round::RoundScheduleStrategy;
pub use tree::TreeScheduleStrategy;

// type ScheduleStrategyImpl = PriorityScheduleStrategy;
// type ScheduleStrategyImpl = TreeScheduleStrategy;
type ScheduleStrategyImpl = CfsScheduleStrategy;

pub struct Scheduler<S: SchedStrategy> {
    current: KRef<ThreadControlBlock>,
    ready_queue: S::Queue,
    exits: VecDeque<KRef<ThreadControlBlock>>,
    reaper: Option<KRef<ThreadControlBlock>>,
    _sched_strategy: PhantomData<S>,
}

impl<S: SchedStrategy> Scheduler<S> {
    fn new() -> Self {
        Self {
            current: IDLE.clone(),
            ready_queue: S::Queue::new(),
            exits: VecDeque::with_capacity(32),
            reaper: Some(REAPER.clone()),
            _sched_strategy: PhantomData,
        }
    }

    pub fn enqueue_task(&mut self, entity: KRef<ThreadControlBlock>) {
        assert_eq!(entity.get_state(), ThreadStateValue::Ready);
        self.ready_queue.push(entity);
    }
    pub fn get_clear_need_schedule(&self) -> bool {
        self.current.get_clear_need_schedule()
    }

    pub fn current_tcb(&self) -> &KRef<ThreadControlBlock> {
        &self.current
    }

    pub fn wake_reaper(&mut self) {
        match self.reaper.take() {
            None => {}
            Some(reaper) => {
                reaper.state.store(ThreadStateValue::Ready);
                self.enqueue_task(reaper);
            }
        }
    }

    pub fn block_reaper(&mut self) {
        assert_eq!(self.current.tid, REAPER.tid);
        self.current.state.store(ThreadStateValue::Blocked);
        self.reaper = Some(REAPER.clone());
    }

    pub fn clean_exits(&mut self, mut handler: impl FnMut(KRef<ThreadControlBlock>)) {
        while !self.exits.is_empty() {
            let t = self.exits.pop_front();
            if let Some(t) = t {
                handler(t);
            }
        }
    }

    /// before call this function, the thread state must already be set
    fn schedule(&mut self) {
        let current_state = self.current.get_state();
        let new = match current_state {
            ThreadStateValue::Ready => {
                info!("current thread is ready");
                IDLE.clone()
            }

            ThreadStateValue::Running => {
                let next = S::pick_with(&mut self.ready_queue, self.current.clone());
                if next.tid != self.current.tid {
                    self.current.state.store(ThreadStateValue::Ready);
                }
                next
            }

            ThreadStateValue::Blocked => {
                S::pick_next(&mut self.ready_queue).unwrap_or(IDLE.clone())
            }

            ThreadStateValue::Zombied => {
                self.exits.push_back(self.current.clone());
                self.wake_reaper();
                S::pick_next(&mut self.ready_queue).unwrap_or(IDLE.clone())
            }
        };

        if new.tid == self.current.tid {
            return;
        }

        Self::context_switch(&mut self.current, new);
    }

    fn context_switch(current: &mut KRef<ThreadControlBlock>, new: KRef<ThreadControlBlock>) {
        let old_ctx = current.context.get();
        let new_ctx = new.context.get();
        unsafe {
            if !current.process.mem.share_same_page_table(&new.process.mem) {
                new.process.mem.activate()
            }
            write_rsp0(new.kstack.top());
            update_per_cpu(|per_cpu| {
                per_cpu.kernel_stack_top = new.kstack.top().as_mut_ptr();
                per_cpu.temp_rsp = core::ptr::null_mut();
            });
            new.set_state(ThreadStateValue::Running);
            *current = new;
            __switch_context(old_ctx, new_ctx);
        }
    }
}

lazy_static! {
    pub static ref SCHEDULER: IrqLocked<Scheduler<ScheduleStrategyImpl>> =
        unsafe { IrqLocked::new(Scheduler::new()) };
}

fn jump_to_idle() -> ! {
    let ctx = IDLE.context.get();
    unsafe { asm!("jmp __switch_to", in("rsi") ctx, options(nostack, noreturn)) }
}

pub fn schedule() {
    SCHEDULER.access(|s| s.schedule());
}

pub fn first_schedule() -> ! {
    IDLE.state.store(ThreadStateValue::Running);
    jump_to_idle()
}

pub fn current_tcb() -> KRef<ThreadControlBlock> {
    SCHEDULER.access(|s| s.current_tcb().clone())
}

pub fn scheduler_debug_info(tf: Option<&InterruptStackFrame>) {
    info!("current running task: {:?}", current_tcb().tid);
    // SCHEDULER
    //     .deref()
    //     .access(|s| info!("scheduler queue: {:?}", s.ready_queue));
    if let Some(f) = tf {
        info!("rip: {:0x?}", f.instruction_pointer);
        info!("rsp: {:0x?}", f.stack_pointer);
        info!("cs: {:?}", f.code_segment);
        info!("rflags: {:?}", f.cpu_flags);
    }
}

pub struct Dequeue<T>(VecDeque<T>);
impl<T> Dequeue<T> {
    pub fn with_capacity(capacity: usize) -> Self {
        Dequeue(VecDeque::with_capacity(capacity))
    }
}

impl ReadyQueue for Dequeue<KRef<ThreadControlBlock>> {
    fn push(&mut self, t: KRef<ThreadControlBlock>) {
        self.0.push_back(t);
    }

    fn pop(&mut self) -> Option<KRef<ThreadControlBlock>> {
        self.0.pop_front()
    }
    fn peek(&self) -> Option<&ThreadControlBlock> {
        self.0.front().map(|t| t.deref())
    }
    fn new() -> Self {
        Dequeue(VecDeque::with_capacity(32))
    }
}
