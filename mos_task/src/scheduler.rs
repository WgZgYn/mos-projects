mod cfs;
mod fcfs;
mod hybrid;
mod priority;
mod round;
mod tree;

use crate::thread::{REAPER, ThreadControlBlock, ThreadStateValue};
use crate::traits::{ReadyQueue, SchedDecision, SchedStrategy};
use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::arch::asm;
use core::marker::PhantomData;
use core::ops::DerefMut;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering};
use lazy_static::lazy_static;
use log::info;
use mos_arch::asm::__switch_context;
use mos_arch::percpu::{current_raw, current_raw_mut};
use mos_arch::sync::{SpinIrqLock, SpinIrqLockGuard};
use mos_lib::sync::{KRef, KWake};
use x86_64::structures::idt::InterruptStackFrame;

type ScheduleStrategyImpl = hybrid::HybridSchedStrategy<cfs::CfsScheduleStrategy>;

lazy_static! {
    pub static ref SCHEDULER: SpinIrqLock<Scheduler<ScheduleStrategyImpl>> =
        SpinIrqLock::new(Scheduler::new());
}

static PREEMPT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub struct Scheduler<S: SchedStrategy> {
    ready_queue: S::Queue,
    exits: VecDeque<KRef<ThreadControlBlock>>,
    reaper: Option<KRef<ThreadControlBlock>>,
    _sched_strategy: PhantomData<S>,
}

impl<S: SchedStrategy> Scheduler<S> {
    fn new() -> Self {
        Self {
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
        current_tcb().get_clear_need_schedule()
    }

    pub fn wake_reaper(&mut self) {
        match self.reaper.take() {
            None => {}
            Some(reaper) => {
                reaper.set_state(ThreadStateValue::Ready);
                self.enqueue_task(reaper);
            }
        }
    }

    pub fn block_reaper(&mut self) {
        assert_eq!(current_tcb().tid, REAPER.tid);
        current_tcb().set_state(ThreadStateValue::Blocked);
        self.reaper = Some(REAPER.clone());
    }

    pub fn clean_exits(&mut self) -> Vec<KRef<ThreadControlBlock>> {
        self.exits.drain(..).collect()
    }

    /// before call this function, the thread state must already be set
    fn schedule(&mut self) -> Option<KRef<ThreadControlBlock>> {
        let current = current_tcb();
        let current_state = current.get_state();
        let new = match current_state {
            ThreadStateValue::Ready => panic!("should not be ready"),
            ThreadStateValue::Running => {
                let SchedDecision::SwitchTo(next) =
                    S::pick_with(&mut self.ready_queue, current.clone())
                else {
                    return None;
                };
                current.set_state(ThreadStateValue::Ready);
                next
            }
            ThreadStateValue::Blocked => {
                S::pick_next(&mut self.ready_queue).expect("should has IDLE task")
            }
            ThreadStateValue::Zombied => {
                self.exits.push_back(current.clone());
                self.wake_reaper();
                S::pick_next(&mut self.ready_queue).expect("should has IDLE task")
            }
        };
        Some(new)
    }

    fn context_switch(old: KRef<ThreadControlBlock>, new: KRef<ThreadControlBlock>) {
        let old_ctx = old.context.get();
        let new_ctx = new.context.get();
        #[inline(never)]
        fn inner(old: KRef<ThreadControlBlock>, new: KRef<ThreadControlBlock>) {
            unsafe {
                if !old.process.mem.share_same_page_table(&new.process.mem) {
                    new.process.mem.activate()
                }
                let percpu = current_raw_mut();
                percpu.kernel_stack_top = new.kernel_stack_top().as_mut_ptr();
                percpu.temp_rsp = core::ptr::null_mut();
                percpu.update_rsp0(new.kernel_stack_top());
            }
        }
        inner(old, new);
        unsafe {
            __switch_context(old_ctx, new_ctx);
        }
    }
}

pub enum ParkDecision<L> {
    Park(L),
    Unpark,
}

pub fn inc_preempt_count() {
    PREEMPT_COUNT.fetch_add(1, Ordering::AcqRel);
}

pub fn dec_preempt_count() {
    assert_ne!(
        PREEMPT_COUNT.fetch_sub(1, Ordering::AcqRel),
        0,
        "should not be zero"
    );
}

pub fn get_preempt_count() -> usize {
    PREEMPT_COUNT.load(Ordering::Acquire)
}

pub fn schedule() {
    let mut guard = SCHEDULER.lock();
    let Some(new) = guard.schedule() else {
        return;
    };
    let old = current_tcb();
    set_current_tcb(new.clone());
    new.set_state(ThreadStateValue::Running);
    drop(guard);
    Scheduler::<ScheduleStrategyImpl>::context_switch(old, new);
}

pub fn first_schedule() -> ! {
    fn jump_to_idle() -> ! {
        let ctx = idle_task().context.get();
        unsafe { asm!("jmp __switch_to", in("rsi") ctx, options(nostack, noreturn)) }
    }

    let idle = idle_task();
    idle.set_state(ThreadStateValue::Running);
    unsafe {
        current_raw_mut().current_tcb = KRef::into_ptr(idle).as_ptr() as _;
    }
    jump_to_idle()
}

pub fn current_tcb() -> KRef<ThreadControlBlock> {
    unsafe {
        let ptr =
            NonNull::new(current_raw().current_tcb as _).expect("current_tcb should not be null");
        KRef::increment_strong_count(ptr);
        KRef::from_ptr(ptr)
    }
}

pub fn idle_task() -> KRef<ThreadControlBlock> {
    unsafe {
        let ptr =
            NonNull::new(current_raw().idle_tcb as _).expect("current_tcb should not be null");
        KRef::increment_strong_count(ptr);
        KRef::from_ptr(ptr)
    }
}

fn set_current_tcb(tcb: KRef<ThreadControlBlock>) {
    unsafe {
        let ptr = NonNull::new(current_raw().current_tcb as *mut ThreadControlBlock)
            .expect("current_tcb should not be null");
        KRef::decrement_strong_count(ptr);
        current_raw_mut().current_tcb = KRef::into_ptr(tcb).as_ptr() as _;
    }
}

fn schedule_and_switch<T: SchedStrategy>(mut guard: SpinIrqLockGuard<Scheduler<T>>) {
    let Some(new) = guard.schedule() else {
        return;
    };
    let old = current_tcb();
    set_current_tcb(new.clone());
    new.set_state(ThreadStateValue::Running);
    drop(guard);
    Scheduler::<ScheduleStrategyImpl>::context_switch(old, new);
}

pub fn wake_blocked_task(provider: impl IntoIterator<Item = KRef<ThreadControlBlock>>) {
    let mut guard = SCHEDULER.lock();
    for task in provider.into_iter() {
        if task
            .trans_state(ThreadStateValue::Blocked, ThreadStateValue::Ready)
            .is_ok()
        {
            guard.enqueue_task(task);
        }
    }
}

pub fn park_current<L>(register: impl FnOnce(&KRef<ThreadControlBlock>) -> ParkDecision<L>) {
    park_with_scheduler(|_s| register(&current_tcb()))
}

pub fn park_with_scheduler<L>(
    register: impl FnOnce(&mut Scheduler<ScheduleStrategyImpl>) -> ParkDecision<L>,
) {
    let mut guard = SCHEDULER.lock();
    match register(guard.deref_mut()) {
        ParkDecision::Park(lock) => {
            current_tcb().set_state(ThreadStateValue::Blocked);
            drop(lock);
            schedule_and_switch(guard);
        }
        ParkDecision::Unpark => {}
    };
}

pub fn exit_current() -> ! {
    let task = current_tcb();
    assert_eq!(task.get_state(), ThreadStateValue::Running);
    let guard = SCHEDULER.lock();
    task.set_state(ThreadStateValue::Zombied);
    task.process.clean_exit_thread(task.tid);
    drop(task);
    schedule_and_switch(guard);
    unreachable!("return from exit_current_task");
}

impl KWake for ThreadControlBlock {
    fn wake(this: KRef<Self>) {
        wake_blocked_task([this]);
    }
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
    fn peek(&self) -> Option<&KRef<ThreadControlBlock>> {
        self.0.front()
    }
    fn new() -> Self {
        Dequeue(VecDeque::with_capacity(32))
    }
}
