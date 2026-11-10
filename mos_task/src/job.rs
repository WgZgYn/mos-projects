//! Session/group identity and the single physical terminal association.
//! One lock makes membership checks and foreground-group changes atomic.
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use core::sync::atomic::Ordering::Relaxed;
use mos_abi::error::{KError, KResult};
use crate::process::SharedState;

struct ControllingTerminal {
    /// A weak session-leader record gives the association a real lifetime:
    /// once the leader and its wait record disappear, tty0 can be acquired by
    /// a later session without retaining the old process forever.
    session_leader: Weak<SharedState>,
    foreground: u64,
}

struct JobState {
    processes: Vec<Weak<SharedState>>,
    terminal: Option<ControllingTerminal>,
}

static JOBS: spin::Mutex<JobState> = spin::Mutex::new(JobState {
    processes: Vec::new(), terminal: None,
});

pub fn group_of(state: &SharedState) -> u64 {
    let _jobs = JOBS.lock();
    state.pgid.load(Relaxed)
}

/// Snapshot strong references while membership is serialized. Drop JOBS
/// before publishing signals: signal delivery may wake a scheduler waiter.
pub fn signal_targets(group: u64) -> Vec<mos_lib::sync::KRef<crate::thread::ThreadControlBlock>> {
    let jobs = JOBS.lock();
    jobs.processes.iter().filter_map(Weak::upgrade)
        .filter(|state| state.pgid.load(Relaxed) == group)
        .filter_map(|state| crate::thread::ThreadControlBlock::find_user_target(
            state.child_pid.as_u64(), false)).collect()
}

pub(crate) fn register(state: &Arc<SharedState>, parent: Option<&Arc<SharedState>>) {
    let mut jobs = JOBS.lock();
    jobs.processes.retain(|entry| entry.strong_count() != 0);
    let (sid, pgid) = parent.map_or((state.child_pid.as_u64(), state.child_pid.as_u64()),
        |parent| (parent.sid.load(Relaxed), parent.pgid.load(Relaxed)));
    state.sid.store(sid, Relaxed);
    state.pgid.store(pgid, Relaxed);
    jobs.processes.push(Arc::downgrade(state));
}

pub fn on_exec(state: &SharedState) {
    let _jobs = JOBS.lock();
    state.executed.store(true, Relaxed);
}

pub fn identity(pid: i32, session: bool) -> KResult<u64> {
    if pid < 0 { return Err(KError::NoProcess); }
    let current = crate::current_tcb();
    let id = if pid == 0 { current.process.pid.as_u64() } else { pid as u64 };
    let jobs = JOBS.lock();
    let state = jobs.processes.iter().filter_map(Weak::upgrade)
        .find(|state| state.child_pid.as_u64() == id).ok_or(KError::NoProcess)?;
    Ok(if session { state.sid.load(Relaxed) } else { state.pgid.load(Relaxed) })
}

pub fn setsid() -> KResult<u64> {
    let current = crate::current_tcb();
    let pid = current.process.pid.as_u64();
    let jobs = JOBS.lock();
    // A group may outlive its original leader. Zombie wait records retain
    // identity too; weak registry entries never pin a whole process alive.
    if jobs.processes.iter().filter_map(Weak::upgrade)
        .any(|state| state.pgid.load(Relaxed) == pid) {
        return Err(KError::OperationNotPermitted);
    }
    current.process.shared.sid.store(pid, Relaxed);
    current.process.shared.pgid.store(pid, Relaxed);
    drop(jobs);
    wake_parent(&current.process);
    Ok(pid)
}

pub fn setpgid(pid: i32, pgid: i32) -> KResult<u64> {
    if pgid < 0 { return Err(KError::InvalidArg); }
    if pid < 0 { return Err(KError::NoProcess); }
    let caller = crate::current_tcb();
    let id = if pid == 0 { caller.process.pid.as_u64() } else { pid as u64 };
    let target = crate::thread::ThreadControlBlock::find_user_target(id, false)
        .ok_or(KError::NoProcess)?;
    let group = if pgid == 0 { id } else { pgid as u64 };
    let jobs = JOBS.lock();
    let state = &target.process.shared;
    if id != caller.process.pid.as_u64() {
        if target.process.ppid.load(Relaxed) != caller.process.pid.as_u64() {
            return Err(KError::NoProcess);
        }
        if state.executed.load(Relaxed) { return Err(KError::NoPermission); }
    }
    let sid = caller.process.shared.sid.load(Relaxed);
    if state.sid.load(Relaxed) != sid || state.sid.load(Relaxed) == id {
        return Err(KError::OperationNotPermitted);
    }
    if group != id && !jobs.processes.iter().filter_map(Weak::upgrade).any(|member|
        member.pgid.load(Relaxed) == group && member.sid.load(Relaxed) == sid) {
        return Err(KError::OperationNotPermitted);
    }
    state.pgid.store(group, Relaxed);
    drop(jobs);
    // A parent waiting for the old group must rescan even if no child exits.
    // Never hold JOBS while acquiring the wait queue (wait4 reads groups there).
    wake_parent(&target.process);
    Ok(0)
}

/// Give tty0 to the first userspace session created by the kernel. Normal
/// userspace acquisition still goes through TIOCSCTTY; this bootstrap is
/// needed because MOS currently installs fd 0/1/2 before init can open tty0.
pub fn bootstrap_terminal(state: &Arc<SharedState>) {
    let mut jobs = JOBS.lock();
    let occupied = jobs.terminal.as_ref()
        .and_then(|terminal| terminal.session_leader.upgrade()).is_some();
    if !occupied {
        jobs.terminal = Some(ControllingTerminal {
            session_leader: Arc::downgrade(state),
            foreground: state.pgid.load(Relaxed),
        });
    }
}

/// TIOCSCTTY for MOS's one physical tty. Stealing an occupied terminal is
/// deliberately unsupported until credentials and hangup delivery exist.
pub fn attach_terminal(state: &Arc<SharedState>, steal: bool) -> KResult<()> {
    if steal { return Err(KError::OperationNotPermitted); }
    let pid = state.child_pid.as_u64();
    if state.sid.load(Relaxed) != pid { return Err(KError::OperationNotPermitted); }
    let mut jobs = JOBS.lock();
    if let Some(terminal) = jobs.terminal.as_ref()
        && let Some(leader) = terminal.session_leader.upgrade() {
        if leader.child_pid == state.child_pid { return Ok(()); }
        return Err(KError::OperationNotPermitted);
    }
    jobs.terminal = Some(ControllingTerminal {
        session_leader: Arc::downgrade(state), foreground: state.pgid.load(Relaxed),
    });
    Ok(())
}

fn controlling_session(jobs: &JobState, state: &SharedState) -> KResult<u64> {
    let leader = jobs.terminal.as_ref().and_then(|terminal| terminal.session_leader.upgrade())
        .ok_or(KError::NotTty)?;
    let session = leader.child_pid.as_u64();
    if state.sid.load(Relaxed) != session { return Err(KError::NotTty); }
    Ok(session)
}

pub fn terminal_session(state: &SharedState) -> KResult<u64> {
    let jobs = JOBS.lock();
    controlling_session(&jobs, state)
}

pub fn terminal_foreground(state: &SharedState) -> KResult<u64> {
    let jobs = JOBS.lock();
    controlling_session(&jobs, state)?;
    Ok(jobs.terminal.as_ref().unwrap().foreground)
}

pub fn set_terminal_foreground(state: &SharedState, group: i32) -> KResult<()> {
    if group <= 0 { return Err(KError::InvalidArg); }
    let mut jobs = JOBS.lock();
    let session = controlling_session(&jobs, state)?;
    let group = group as u64;
    if !jobs.processes.iter().filter_map(Weak::upgrade).any(|member|
        member.pgid.load(Relaxed) == group && member.sid.load(Relaxed) == session) {
        return Err(KError::OperationNotPermitted);
    }
    jobs.terminal.as_mut().unwrap().foreground = group;
    Ok(())
}

/// Input has no calling process, so it snapshots tty0's foreground group.
/// A dead session leader makes the terminal unowned instead of reusing a
/// numeric SID that might eventually identify an unrelated process.
pub fn terminal_foreground_for_input() -> Option<u64> {
    let jobs = JOBS.lock();
    let terminal = jobs.terminal.as_ref()?;
    terminal.session_leader.upgrade()?;
    let group = terminal.foreground;
    if jobs.processes.iter().filter_map(Weak::upgrade).any(|member|
        member.pgid.load(Relaxed) == group && !member.exited.load(Relaxed)) {
        Some(group)
    } else {
        // Keep the session association: its shell may install another group.
        None
    }
}

fn wake_parent(process: &crate::process::ProcessControlBlock) {
    let parent_queue = process.parent_exit_queue.lock().clone();
    if let Some(queue) = parent_queue {
        let waiters: Vec<_> = queue.lock().wake_all().into_iter().collect();
        crate::scheduler::wake_blocked_task(waiters);
    }
}
