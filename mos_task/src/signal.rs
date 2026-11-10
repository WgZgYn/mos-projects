//! Process dispositions and thread masks/pending; delivery lives in mos_syscall.
use mos_abi::signal::{KernelSigAction, SIGNAL_COUNT, SIG_IGN};

#[derive(Clone)]
pub struct ProcessSignals {
    pub actions: [KernelSigAction; SIGNAL_COUNT],
    pub pending: u64,
}

impl Default for ProcessSignals {
    fn default() -> Self {
        Self { actions: [KernelSigAction::default(); SIGNAL_COUNT], pending: 0 }
    }
}

impl ProcessSignals {
    pub fn fork_child(&self) -> Self {
        // A child inherits dispositions, but starts with no pending signals.
        Self { actions: self.actions, pending: 0 }
    }

    pub fn on_exec(&mut self) {
        // Ignored signals stay ignored; caught/default dispositions are reset.
        // Pending signals survive exec, as does the surviving thread's mask.
        for action in &mut self.actions {
            if action.handler != SIG_IGN { *action = KernelSigAction::default(); }
        }
    }
}

pub struct ThreadSignals {
    pub interrupt_wait: Option<alloc::sync::Arc<InterruptWait>>,
    pub blocked: u64,
    pub pending: u64,
    pub pending_code: [i32; SIGNAL_COUNT],
    pub pending_pid: [i32; SIGNAL_COUNT],
    pub pending_status: [i32; SIGNAL_COUNT],
}

impl Default for ThreadSignals {
    fn default() -> Self {
        Self { interrupt_wait: None, blocked: 0, pending: 0, pending_code: [0; SIGNAL_COUNT],
            pending_pid: [0; SIGNAL_COUNT], pending_status: [0; SIGNAL_COUNT] }
    }
}

/// Identity and lifetime of one interruptible park. An old signal waker must
/// not wake a later, unrelated sleep by looking only at the thread's state.
pub struct InterruptWait {
    pub(crate) active: core::sync::atomic::AtomicBool,
    pub(crate) task: mos_lib::sync::KRef<crate::thread::ThreadControlBlock>,
}

impl alloc::task::Wake for InterruptWait {
    fn wake(self: alloc::sync::Arc<Self>) {
        crate::scheduler::wake_interruptible(&self);
    }
}

fn ignored(signal: i32, action: KernelSigAction) -> bool {
    use mos_abi::signal::SIG_DFL;
    action.handler == SIG_IGN
        || (action.handler == SIG_DFL && matches!(signal, 17 | 23 | 28))
}

/// Publish one standard signal after the caller has resolved its target.
/// The pending bit and interruptible-wait wakeup live in the task layer so
/// device input can generate terminal signals without depending on syscalls.
pub fn publish_standard(thread: &crate::thread::ThreadControlBlock, signal: i32,
    sender_pid: i32, code: i32) {
    use mos_abi::signal::signal_bit;
    // wait4 holds this lock from pending inspection until it has become
    // Blocked. Publish first, drain its wait entry, then acquire the scheduler:
    // this closes the check-to-sleep gap without a queue/scheduler inversion.
    let mut queue = thread.process.child_exit_queue.lock();
    let process = thread.process.signals.lock();
    let action = process.actions[(signal - 1) as usize];
    if action.handler == SIG_IGN { return; }
    let mut local = thread.signals.lock();
    let bit = signal_bit(signal);
    // Standard signals coalesce; retain the first source information.
    if local.pending & bit == 0 {
        local.pending_code[(signal - 1) as usize] = code;
        local.pending_pid[(signal - 1) as usize] = sender_pid;
        local.pending_status[(signal - 1) as usize] = 0;
    }
    local.pending |= bit;
    let wake = local.blocked & bit == 0 && !ignored(signal, action);
    let interrupt_wait = if wake { local.interrupt_wait.clone() } else { None };
    drop(local);
    drop(process);
    let waiters: alloc::vec::Vec<_> = if wake {
        queue.wake_all().into_iter().collect()
    } else { alloc::vec::Vec::new() };
    drop(queue);
    crate::scheduler::wake_blocked_task(waiters);
    if let Some(wait) = interrupt_wait { crate::scheduler::wake_interruptible(&wait); }
}

/// Returns false if an actionable signal prevents sleeping. The resource's
/// lock must span inspection through register, as for ordinary park_current.
pub fn park_interruptible<L>(register: impl FnOnce(
    &mos_lib::sync::KRef<crate::thread::ThreadControlBlock>) -> crate::scheduler::ParkDecision<L>) -> bool {
    park_interruptible_with_waker(|task, _| register(task))
}

/// External event sources must retain this per-park waker, never a raw TCB
/// waker. Once the park ends even an already-dequeued event becomes harmless.
pub fn park_interruptible_with_waker<L>(register: impl FnOnce(
    &mos_lib::sync::KRef<crate::thread::ThreadControlBlock>, core::task::Waker)
    -> crate::scheduler::ParkDecision<L>) -> bool {
    use alloc::sync::Arc;
    use core::sync::atomic::{AtomicBool, Ordering};
    use crate::scheduler::{park_with_scheduler, ParkDecision};
    use mos_abi::signal::{SIG_DFL, SIGCHLD, signal_bit};
    let task = crate::current_tcb();
    let token = Arc::new(InterruptWait { active: AtomicBool::new(true), task: task.clone() });
    let mut interrupted = false;
    park_with_scheduler(|_| {
        let process = task.process.signals.lock();
        let mut local = task.signals.lock();
        let ready = (process.pending | local.pending) & !local.blocked;
        let pending = (1..=64).any(|signal| {
            let action = process.actions[(signal - 1) as usize];
            ready & signal_bit(signal) != 0 && action.handler != SIG_IGN
                && !(action.handler == SIG_DFL && matches!(signal, SIGCHLD | 23 | 28))
        });
        if pending {
            interrupted = true;
            return ParkDecision::Unpark;
        }
        local.interrupt_wait = Some(token.clone());
        drop(local);
        drop(process);
        // Sender publishes under signals.lock, then obtains the scheduler
        // lock: it cannot wake us before the Blocked transition below.
        register(&task, core::task::Waker::from(token.clone()))
    });
    token.active.store(false, Ordering::Release);
    let mut local = task.signals.lock();
    if local.interrupt_wait.as_ref().is_some_and(|wait| Arc::ptr_eq(wait, &token)) {
        local.interrupt_wait = None;
    }
    !interrupted
}
