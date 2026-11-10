use mos_arch::sync::SpinIrqLock;
use crate::process::{KERNEL, KernelStack, ProcessControlBlock};
use crate::scheduler::{ParkDecision, SCHEDULER, park_with_scheduler};
use core::cell::UnsafeCell;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, AtomicUsize, Ordering};
use intrusive_collections::RBTreeLink;
use lazy_static::lazy_static;
use log::{info, trace};
use mos_arch::asm::intr_exit_point;
use mos_arch::structs::{FpuState, InterruptStack, IretRegisters, ThreadContext};
use mos_arch::{enable_and_halt, init_fpu_state};
use mos_lib::sync::{KRef, RefCounted};
use mos_mem::slab::Slab;
use num_enum::TryFromPrimitive;
use x86_64::VirtAddr;

const REAPER_PRIORITY: u8 = 5;
pub const DEFAULT_THREAD_PRIORITY: u8 = 2;

static TCB_SLAB: SpinIrqLock<Slab<ThreadControlBlock>> = SpinIrqLock::new(Slab::new());

pub fn sti_hlt_loop() -> ! {
    info!("STI HLT loop");
    loop {
        enable_and_halt();
    }
}

pub fn reaper() -> ! {
    info!("[REAPER]: first running with {:?}", REAPER.tid);
    loop {
        let tasks = SCHEDULER.lock().clean_exits();
        for task in tasks {
            info!("[REAPER]: drop task {:?}", task.tid);
        }
        park_with_scheduler(|s| {
            s.block_reaper();
            ParkDecision::Park(())
        });
    }
}

lazy_static! {
    pub static ref REAPER: KRef<ThreadControlBlock> = {
        let t = ProcessControlBlock::kernel_func(KERNEL.clone(), reaper, Priority(REAPER_PRIORITY));
        t.state.store(ThreadStateValue::Blocked);
        t
    };
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Tid(u64);
impl Tid {
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// before calling the schedule function, ThreadState need to be set
/// it acts a part of the state machine
#[derive(Debug)]
pub struct ThreadState(AtomicU8);

/// a translation of the ThreadState AtomicU8
#[repr(u8)]
#[derive(TryFromPrimitive, PartialEq, Eq, Copy, Clone, Debug)]
pub enum ThreadStateValue {
    Ready,
    Running,
    Blocked,
    Zombied,
}

impl ThreadState {
    pub fn new(state: ThreadStateValue) -> Self {
        Self(AtomicU8::new(state as u8))
    }
    pub fn store(&self, state: ThreadStateValue) {
        self.0.store(state as u8, Ordering::SeqCst);
    }
    pub fn load(&self) -> ThreadStateValue {
        let n = self.0.load(Ordering::SeqCst);
        n.try_into()
            .expect("ThreadState value cannot be converted into a ThreadStateValue")
    }

    /// if current state is `current`, set the new state, return old state
    /// if current state is not `current`, not set the new state, return current state  
    pub fn compare_exchange(
        &self,
        current: ThreadStateValue,
        new: ThreadStateValue,
    ) -> Result<ThreadStateValue, ThreadStateValue> {
        match self.0.compare_exchange(
            current as u8,
            new as u8,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(old) => Ok(ThreadStateValue::try_from(old).expect("illegal state")),
            Err(current) => Err(ThreadStateValue::try_from(current).expect("illegal state")),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Copy, Clone, Debug)]
pub struct Priority(pub u8);

impl Default for Priority {
    fn default() -> Self {
        Priority(DEFAULT_THREAD_PRIORITY)
    }
}

#[derive(Debug, Default)]
pub struct VRunTime(AtomicU64);

impl VRunTime {
    pub fn load(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

unsafe impl Send for ThreadControlBlock {}
unsafe impl Sync for ThreadControlBlock {}

pub struct ThreadControlBlock {
    /// unique thread id
    pub tid: Tid,

    /// lifecycle state
    /// It can be a mark for destroy or a hint of the thread state.
    /// It also acts like schedule state which is managed by scheduler.
    /// change mostly by syscall like sys_exit or sys_sleep
    /// it will schedule immediately
    state: ThreadState,

    /// parent process
    /// because the thread rely on the address space and other resource,
    /// it needs strong reference
    pub process: KRef<ProcessControlBlock>,

    /// the pointer is rsp which points to kernel context
    pub(crate) context: UnsafeCell<ThreadContext>,

    /// the kernel stack
    /// set to TSS.RSP0
    /// receive the TrapFrame, support the kernel function call stack
    kstack: KernelStack,

    /// used for preempt schedule
    /// for example the timer interrupt will set this if time-slice over
    /// before interrupt exit it will be checked and do preempt-schedule
    need_sched: AtomicBool,

    /// used for priority-driven schedule
    pub priority: Priority,

    /// address passed to set_tid_address(2); on exit the kernel clears the
    /// futex there and wakes waiters (futex part lands with phase 3)
    clear_child_tid: AtomicU64,

    /// Linux masks are per-thread, unlike the process-wide disposition table.
    pub signals: spin::Mutex<crate::signal::ThreadSignals>,

    /// used for cfs schedule
    vruntime: VRunTime,
    pub(crate) node: RBTreeLink,

    /// used for KRef
    refcount: AtomicUsize,
}

impl ThreadControlBlock {
    /// Find a live user target and acquire ownership before releasing the slab.
    /// The last KRef can reach zero before destroy obtains this lock: never
    /// resurrect that entry, even though its storage has not been removed yet.
    pub fn find_user_target(id: u64, by_tid: bool) -> Option<KRef<Self>> {
        let slab = TCB_SLAB.lock();
        let target = slab.iter().find(|t| {
            (if by_tid { t.tid.as_u64() } else { t.process.pid.as_u64() }) == id
                && t.process.mem.is_userspace()
                && !t.process.shared.exited.load(Ordering::Acquire)
                && t.get_state() != ThreadStateValue::Zombied
        })?;
        target.refcount.fetch_update(Ordering::Acquire, Ordering::Relaxed,
            |count| if count == 0 { None } else { count.checked_add(1) }).ok()?;
        // SAFETY: slab lock protects storage; the successful nonzero increment
        // now owns one strong reference, consumed by the returned KRef.
        Some(unsafe { KRef::from_ptr(NonNull::from_ref(target)) })
    }

    /// this function just fill the struct field and init the intr restore kernel context
    /// not call `fninit`
    pub fn create_with_context(
        state: ThreadState,
        process: KRef<ProcessControlBlock>,
        context: InterruptStack,
        fpu_state: FpuState,
        kstack: KernelStack,
        priority: Priority,
    ) -> KRef<Self> {
        let mut kernel_context = ThreadContext {
            rflags: process.context.rflags.bits(),
            fpu: fpu_state,
            ..Default::default()
        };

        // SAFETY: kstack.top() returns a valid pointer within the kernel stack allocation.
        // trap and krip are within bounds (top - sizeof(InterruptStack) - 8 > stack bottom).
        unsafe {
            let ktop = kstack.top().as_mut_ptr::<u64>();
            let trap = ktop.cast::<InterruptStack>().sub(1);
            trap.write(context);
            let krip = trap.cast::<u64>().sub(1);
            krip.write(intr_exit_point());
            kernel_context.rsp = krip as _;
        }

        let mut guard = TCB_SLAB.lock();
        let tcb = guard.insert_with_key(|key| {
            let tid = Tid(key as _);
            process.threads.lock().push(tid);
            Self {
                tid,
                state,
                process,
                context: UnsafeCell::new(kernel_context),
                kstack,
                need_sched: AtomicBool::new(false),
                priority,
                clear_child_tid: AtomicU64::new(0),
                signals: spin::Mutex::new(crate::signal::ThreadSignals::default()),
                vruntime: VRunTime(AtomicU64::new(1024)),
                node: RBTreeLink::new(),
                refcount: AtomicUsize::new(1),
            }
        });
        unsafe { KRef::from_ptr(NonNull::from_ref(tcb)) }
    }

    pub fn create(
        process: KRef<ProcessControlBlock>,
        entry: VirtAddr,
        user_rsp: Option<VirtAddr>,
        kstack: KernelStack,
        priority: Priority,
    ) -> KRef<Self> {
        let iret_rsp = user_rsp.unwrap_or(kstack.top());

        let context = InterruptStack {
            iret: IretRegisters {
                rip: entry.as_u64(),
                rsp: iret_rsp.as_u64(),
                cs: process.context.cs.0 as _,
                ss: process.context.ss.0 as _,
                rflags: process.context.rflags.bits(),
            },
            ..Default::default()
        };

        let mut fpu_state = FpuState::default();
        init_fpu_state(&mut fpu_state);

        Self::create_with_context(
            ThreadState::new(ThreadStateValue::Ready),
            process,
            context,
            fpu_state,
            kstack,
            priority,
        )
    }
    pub fn kernel_stack_top(&self) -> VirtAddr {
        self.kstack.top()
    }
    pub fn set_clear_child_tid(&self, addr: u64) {
        self.clear_child_tid.store(addr, Ordering::Release);
    }
    pub fn clear_child_tid(&self) -> u64 {
        self.clear_child_tid.load(Ordering::Acquire)
    }
    /// Store the user FS base for this thread (called by arch_prctl).
    pub fn set_fs_base(&self, base: u64) {
        unsafe { (*self.context.get()).fs_base = base };
    }
    pub fn fs_base(&self) -> u64 {
        unsafe { (*self.context.get()).fs_base }
    }
    pub fn init_fpu_state(&self) {
        unsafe {
            let fpu = &mut (*self.context.get()).fpu;
            init_fpu_state(fpu);
        }
    }
    pub fn get_state(&self) -> ThreadStateValue {
        self.state.load()
    }
    pub(crate) fn set_state(&self, state: ThreadStateValue) {
        self.state.store(state);
    }
    pub(crate) fn trans_state(
        &self,
        expected_state: ThreadStateValue,
        target_state: ThreadStateValue,
    ) -> Result<ThreadStateValue, ThreadStateValue> {
        self.state.compare_exchange(expected_state, target_state)
    }
    pub fn set_need_schedule(&self) {
        self.need_sched.store(true, Ordering::SeqCst)
    }
    pub fn get_clear_need_schedule(&self) -> bool {
        self.need_sched.swap(false, Ordering::SeqCst)
    }
    pub fn update_vruntime(&self, delta: u64) {
        let old = self.vruntime.load();
        let new = old + delta * (1024 / (self.priority.0 as u64 + 1));
        self.vruntime.0.store(new, Ordering::SeqCst);
    }
    pub fn cfs_key(&self) -> (u64, Tid) {
        (self.vruntime.load(), self.tid)
    }
}

impl Drop for ThreadControlBlock {
    fn drop(&mut self) {
        let tid = self.tid;
        trace!("{:?} ThreadControlBlock::drop", tid);
    }
}

impl RefCounted for ThreadControlBlock {
    fn ref_count(&self) -> &AtomicUsize {
        &self.refcount
    }
    unsafe fn destroy(&self) {
        let tid = self.tid;
        TCB_SLAB.lock().remove_inplace(self.tid.0 as _);
        trace!("removed {:?} from tcb_slab", tid);
    }
}
