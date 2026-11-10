//! Signal control and user-return delivery (legacy x86_64 FXSAVE ABI).
use mos_abi::error::{KError, KResult};
use mos_abi::signal::*;
use mos_mem::virt::MemorySet;
use mos_task::current_tcb;

fn copy_output(memory: &MemorySet, address: u64, bytes: &[u8]) -> KResult<()> {
    if bytes.is_empty() { return Ok(()); }
    let result = memory.copy_to_user(address, bytes);
    // COW may have been resolved before a later output page failed.
    mos_platform::smp::tlb_shootdown();
    result
}

pub fn sys_rt_sigprocmask(how: i32, set: u64, oldset: u64, size: u64) -> KResult<()> {
    if size != SIGSET_BYTES as u64 { return Err(KError::InvalidArg); }
    let thread = current_tcb();
    let memory = &thread.process.mem;
    // Read before writing oldset: Linux allows set and oldset to alias.
    let new_bits = if set != 0 {
        let mut bytes = [0; SIGSET_BYTES];
        memory.copy_from_user(set, &mut bytes)?;
        Some(u64::from_ne_bytes(bytes))
    } else { None };
    let mut signals = thread.signals.lock();
    let old = signals.blocked;
    let new = match new_bits {
        Some(bits) => Some(change_mask(old, bits, how)?),
        None => None, // how is ignored for a pure query.
    };
    if oldset != 0 { copy_output(memory, oldset, &old.to_ne_bytes())?; }
    // Commit only after successful output; EFAULT leaves MOS state unchanged.
    if let Some(mask) = new { signals.blocked = mask; }
    Ok(())
}

pub fn sys_rt_sigaction(signal: i32, act: u64, oldact: u64, size: u64) -> KResult<()> {
    if size != SIGSET_BYTES as u64 || signal_bit(signal) == 0
        || signal == SIGKILL || signal == SIGSTOP {
        return Err(KError::InvalidArg);
    }
    let thread = current_tcb();
    let memory = &thread.process.mem;
    let new_action = if act != 0 {
        let mut bytes = [0; 32];
        memory.copy_from_user(act, &mut bytes)?;
        let mut action = KernelSigAction::from_bytes(bytes);
        // Alternate stacks and automatic child reaping need separate lifecycle
        // support. Never silently accept flags whose effects are unimplemented.
        let supported = SA_RESTORER | SA_SIGINFO | SA_RESTART | SA_NODEFER | SA_RESETHAND;
        if action.flags & !supported != 0
            || (signal == SIGCHLD && action.handler == SIG_IGN) {
            return Err(KError::Unsupported);
        }
        if action.handler > SIG_IGN && (action.flags & SA_RESTORER == 0
            || !memory.user_executable(action.handler) || !memory.user_executable(action.restorer)) {
            return Err(KError::InvalidArg);
        }
        action.mask = blockable(action.mask);
        Some(action)
    } else { None };
    let mut signals = thread.process.signals.lock();
    let index = (signal - 1) as usize;
    if oldact != 0 { copy_output(memory, oldact, &signals.actions[index].to_bytes())?; }
    if let Some(action) = new_action {
        signals.actions[index] = action;
        if action.handler == SIG_IGN {
            signals.pending &= !signal_bit(signal);
            thread.signals.lock().pending &= !signal_bit(signal);
        }
    }
    Ok(())
}

/// PID/TID and process-group targets in the current single-UID/thread subset.
/// All-process broadcasts, stopped tasks and realtime queues follow later.
pub fn sys_send_signal(id: i32, signal: i32, thread_directed: bool) -> KResult<()> {
    if !(0..=64).contains(&signal) { return Err(KError::InvalidArg); }
    if thread_directed && id <= 0 { return Err(KError::InvalidArg); }
    if id == -1 { return Err(KError::Unsupported); } // all-process broadcast follows later
    let sender = current_tcb();
    let targets = if id > 0 {
        alloc::vec![mos_task::thread::ThreadControlBlock::find_user_target(id as u64, thread_directed)
            .ok_or(KError::NoProcess)?]
    } else {
        let group = if id == 0 { mos_task::job::group_of(&sender.process.shared) }
            else { -(id as i64) as u64 };
        mos_task::job::signal_targets(group)
    };
    if targets.is_empty() { return Err(KError::NoProcess); }
    if signal == 0 { return Ok(()); }
    // Default termination must eventually end the whole thread group. Until
    // exit_group and shared pending selection exist, reject that ambiguity.
    if targets.iter().any(|thread| thread.process.threads().lock().len() > 1) {
        return Err(KError::Unsupported);
    }
    if signal >= 32 || matches!(signal, 18..=22) { return Err(KError::Unsupported); }
    for thread in targets {
        mos_task::signal::publish_standard(&thread, signal, sender.process.pid.as_u64() as i32,
            if thread_directed { -6 } else { 0 });
    }
    Ok(())
}

fn ignored(signal: i32, action: KernelSigAction) -> bool {
    action.handler == SIG_IGN
        || (action.handler == SIG_DFL && matches!(signal, SIGCHLD | 23 | 28))
}

/// Publish child state and SIGCHLD before either wait4 or a handler can observe
/// completion. No strong parent reference is retained over the child's life.
pub fn notify_child_exit(child: &mos_task::thread::ThreadControlBlock, status: i32) {
    use core::sync::atomic::Ordering;
    let parent = mos_task::thread::ThreadControlBlock::find_user_target(
        child.process.ppid.load(Ordering::Acquire), false);
    let queue = child.process.parent_exit_queue.lock().clone();
    let mut queue = queue.as_ref().map(|queue| queue.lock());
    let mut interrupt_wait = None;
    child.process.shared.wait_status.store(status, Ordering::Relaxed);
    if let Some(parent) = parent.as_ref() {
        let process = parent.process.signals.lock();
        let action = process.actions[(SIGCHLD - 1) as usize];
        let mut local = parent.signals.lock();
        let bit = signal_bit(SIGCHLD);
        if action.handler != SIG_IGN {
            let index = (SIGCHLD - 1) as usize;
            // Standard signals coalesce, but every child keeps its own wait
            // record. A handler must drain wait4 rather than count signals.
            if local.pending & bit == 0 {
                local.pending_pid[index] = child.process.pid.as_u64() as i32;
                local.pending_code[index] = if status & 0x7f == 0 { 1 } else { 2 };
                local.pending_status[index] = if status & 0x7f == 0 {
                    (status >> 8) & 0xff
                } else { status & 0x7f };
            }
            local.pending |= bit;
            if local.blocked & bit == 0 && !ignored(SIGCHLD, action) {
                interrupt_wait = local.interrupt_wait.clone();
            }
        }
        // Handler delivery takes these signal locks; wait4 takes the queue.
        // Publish exited before releasing either observation path.
        child.process.shared.exited.store(true, Ordering::Release);
    } else {
        child.process.shared.exited.store(true, Ordering::Release);
    }
    let waiters: alloc::vec::Vec<_> = queue.as_mut().map_or_else(alloc::vec::Vec::new,
        |queue| queue.wake_all().into_iter().collect());
    drop(queue);
    mos_task::scheduler::wake_blocked_task(waiters);
    if let Some(wait) = interrupt_wait { mos_task::scheduler::wake_interruptible(&wait); }
}

/// A broken user write generates SIGPIPE for the calling thread. Keep VFS
/// independent of Linux signals: it reports BrokenPipe, the syscall translates
/// that event here. Delivery happens at user return after file/pipe locks drop.
pub fn broken_pipe_signal() {
    let thread = current_tcb();
    let process = thread.process.signals.lock();
    if process.actions[(SIGPIPE - 1) as usize].handler == SIG_IGN { return; }
    let mut local = thread.signals.lock();
    let bit = signal_bit(SIGPIPE);
    if local.pending & bit == 0 {
        local.pending_code[(SIGPIPE - 1) as usize] = 0;
        local.pending_pid[(SIGPIPE - 1) as usize] = thread.process.pid.as_u64() as i32;
    }
    local.pending |= bit;
}

/// Called under child_exit_queue before parking. Ignored or masked signals
/// must not turn an otherwise blocking wait into a stream of spurious EINTR.
pub fn interrupts_wait(thread: &mos_task::thread::ThreadControlBlock) -> bool {
    let process = thread.process.signals.lock();
    let local = thread.signals.lock();
    let ready = (process.pending | local.pending) & !local.blocked;
    (1..=64).any(|signal| ready & signal_bit(signal) != 0
        && !ignored(signal, process.actions[(signal - 1) as usize]))
}

use mos_abi::signal_frame::*;
use mos_arch::structs::{FpuState, InterruptStack, IretRegisters, PreservedRegisters, ScratchRegisters};

pub fn deliver_pending(context: &mut InterruptStack) {
    deliver_syscall_pending(context, None);
}

/// `restart` is supplied only by dispatch for an interrupted, restartable
/// syscall. It is local to this return, never persistent per-thread state that
/// a syscall inside a nested handler could accidentally consume.
pub fn deliver_syscall_pending(context: &mut InterruptStack, restart: Option<u64>) {
    if context.iret.cs & 3 != 3 { return; }
    loop {
        let thread = current_tcb();
        let mut process = thread.process.signals.lock();
        let mut local = thread.signals.lock();
        let ready = (process.pending | local.pending) & !local.blocked;
        if ready == 0 {
            // If the pending reason disappeared or was ignored, no handler
            // needs to expose EINTR: resume the original wait transparently.
            if let Some(number) = restart { restart_syscall(context, number); }
            return;
        }
        let index = ready.trailing_zeros() as usize;
        let signal = index as i32 + 1;
        let bit = signal_bit(signal);
        let code = if local.pending & bit != 0 { local.pending_code[index] } else { 0 };
        let sender_pid = if local.pending & bit != 0 { local.pending_pid[index] } else { 0 };
        let child_status = if local.pending & bit != 0 { local.pending_status[index] } else { 0 };
        local.pending &= !bit;
        process.pending &= !bit;
        let action = process.actions[index];
        if ignored(signal, action) {
            continue;
        }
        if action.handler == SIG_DFL {
            drop(local);
            drop(process);
            drop(thread);
            crate::process::sys_exit_signal(signal);
        }
        let saved_mask = local.blocked;
        local.blocked = blockable(saved_mask | action.mask
            | if action.flags & SA_NODEFER == 0 { bit } else { 0 });
        if action.flags & SA_RESETHAND != 0 { process.actions[index] = KernelSigAction::default(); }
        drop(local);
        drop(process);
        // Save the restart point IN the signal frame, before redirecting RIP
        // to the handler. sigreturn then restores it just like ordinary user
        // context; userspace may still deliberately edit its ucontext.
        if action.flags & SA_RESTART != 0 {
            if let Some(number) = restart { restart_syscall(context, number); }
        }
        let built = build_frame(&thread.process.mem, context, signal, code,
            sender_pid, child_status, action, saved_mask);
        drop(thread);
        if built.is_err() { crate::process::sys_exit_signal(SIGSEGV); }
        return;
    }
}

fn restart_syscall(context: &mut InterruptStack, number: u64) {
    // Both supported MOS syscall entry instructions (0F 05 and CD 80) are
    // two bytes. The hardware-saved RIP points immediately past that opcode.
    // wait4 preserves argument registers, so only RIP and RAX need restoring.
    if let Some(instruction) = context.iret.rip.checked_sub(2) {
        context.iret.rip = instruction;
        context.scratch.rax = number;
    }
}

fn build_frame(memory: &MemorySet, context: &mut InterruptStack, signal: i32,
    code: i32, pid: i32, child_status: i32, action: KernelSigAction, mask: u64) -> KResult<()> {
    if !memory.user_executable(action.handler) || !memory.user_executable(action.restorer) {
        return Err(KError::OutOfBounds);
    }
    let (base, fp_address) = frame_addresses(context.iret.rsp).ok_or(KError::OutOfBounds)?;
    let mut fp = FpuState::default();
    mos_arch::save_fpu_state(&mut fp);
    // No XSAVE extensions are advertised. Zero software-reserved bytes so
    // userspace cannot mistake the legacy image for an extended xstate area.
    fp.bytes_mut()[416..].fill(0);
    let mut bytes = [0u8; FRAME_BYTES];
    write_word(&mut bytes, 0, action.restorer);
    write_word(&mut bytes, UCONTEXT, 6); // UC_SIGCONTEXT_SS | UC_STRICT_RESTORE_SS
    write_word(&mut bytes, UCONTEXT + 24, 2); // uc_stack.ss_flags = SS_DISABLE
    let s = &context.scratch;
    let p = &context.preserved;
    let registers = [s.r8, s.r9, s.r10, s.r11, p.r12, p.r13, p.r14, p.r15,
        s.rdi, s.rsi, p.rbp, p.rbx, s.rdx, s.rax, s.rcx, context.iret.rsp,
        context.iret.rip, context.iret.rflags];
    for (index, value) in registers.into_iter().enumerate() {
        write_word(&mut bytes, MCONTEXT + index * 8, value);
    }
    write_word(&mut bytes, MCONTEXT + 18 * 8, context.iret.cs | context.iret.ss << 48);
    write_word(&mut bytes, MCONTEXT + 21 * 8, mask);
    write_word(&mut bytes, FP_POINTER, fp_address);
    write_word(&mut bytes, SIGMASK, mask);
    bytes[SIGINFO..SIGINFO + 4].copy_from_slice(&signal.to_ne_bytes());
    bytes[SIGINFO + 8..SIGINFO + 12].copy_from_slice(&code.to_ne_bytes());
    bytes[SIGINFO + 16..SIGINFO + 20].copy_from_slice(&(pid as i32).to_ne_bytes());
    if signal == SIGCHLD && matches!(code, 1 | 2) {
        bytes[SIGINFO + 24..SIGINFO + 28].copy_from_slice(&child_status.to_ne_bytes());
    }
    copy_output(memory, fp_address, fp.bytes())?;
    copy_output(memory, base, &bytes)?;
    context.iret.rsp = base;
    context.iret.rip = action.handler;
    context.iret.rflags = restored_flags(context.iret.rflags) & !(1 << 10); // clear DF for C ABI
    context.scratch.rdi = signal as u64;
    context.scratch.rsi = base + SIGINFO as u64;
    context.scratch.rdx = base + UCONTEXT as u64;
    context.scratch.rax = 0;
    Ok(())
}

pub fn sys_rt_sigreturn(context: &mut InterruptStack) {
    if restore_frame(context).is_err() {
        // A forged/broken user frame is a user fault, never an IRET/FXRSTOR
        // exception in ring 0. Do not return -errno into an invalid trampoline.
        crate::process::sys_exit_signal(SIGSEGV);
    }
}

fn restore_frame(context: &mut InterruptStack) -> KResult<()> {
    let thread = current_tcb();
    let memory = &thread.process.mem;
    // Handler RET has already popped pretcode before entering its restorer.
    let base = context.iret.rsp.checked_sub(8).ok_or(KError::OutOfBounds)?;
    let mut bytes = [0; FRAME_BYTES];
    memory.copy_from_user(base, &mut bytes)?;
    let register = |index: usize| read_word(&bytes, MCONTEXT + index * 8);
    let selectors = register(18);
    let user = mos_arch::structs::CpuContext::user_context();
    if selectors & 0xffff != user.cs.0 as u64 || selectors >> 48 != user.ss.0 as u64
        || !memory.user_executable(register(16)) || !user_address(register(15))
        || read_word(&bytes, UCONTEXT) & !6 != 0 {
        return Err(KError::InvalidArg);
    }
    let fp_address = read_word(&bytes, FP_POINTER);
    let mut fp = FpuState::default();
    if fp_address == 0 {
        fp.bytes_mut()[..2].copy_from_slice(&0x037fu16.to_ne_bytes());
        fp.bytes_mut()[24..28].copy_from_slice(&0x1f80u32.to_ne_bytes());
    } else {
        if fp_address & 15 != 0 { return Err(KError::InvalidArg); }
        memory.copy_from_user(fp_address, fp.bytes_mut())?;
        let mut hardware = FpuState::default();
        mos_arch::save_fpu_state(&mut hardware);
        let mask = u32::from_ne_bytes(hardware.bytes()[28..32].try_into().unwrap());
        let mask = if mask == 0 { 0xffbf } else { mask };
        let mxcsr = u32::from_ne_bytes(fp.bytes()[24..28].try_into().unwrap());
        if mxcsr & !mask != 0 || fp.bytes()[464..468] != [0; 4] {
            return Err(KError::InvalidArg);
        }
    }
    let restored = InterruptStack {
        preserved: PreservedRegisters { r15: register(7), r14: register(6), r13: register(5),
            r12: register(4), rbp: register(10), rbx: register(11) },
        scratch: ScratchRegisters { r11: register(3), r10: register(2), r9: register(1),
            r8: register(0), rsi: register(9), rdi: register(8), rdx: register(12),
            rcx: register(14), rax: register(13) },
        iret: IretRegisters { rip: register(16), rsp: register(15), cs: user.cs.0 as u64,
            ss: user.ss.0 as u64, rflags: restored_flags(register(17)) },
    };
    thread.signals.lock().blocked = blockable(read_word(&bytes, SIGMASK));
    *context = restored;
    // Kernel target disables SSE, so the final Rust return path does not
    // consume SIMD registers between FXRSTOR and IRET.
    mos_arch::load_fpu_state(&mut fp);
    Ok(())
}

pub fn sys_rt_sigpending(set: u64, size: u64) -> KResult<()> {
    // Unlike sigprocmask, Linux permits a shorter buffer for sigpending.
    if size > SIGSET_BYTES as u64 { return Err(KError::InvalidArg); }
    let thread = current_tcb();
    // Always take process state before thread state if both locks are needed.
    let process = thread.process.signals.lock();
    let local = thread.signals.lock();
    let pending = (process.pending | local.pending) & local.blocked;
    copy_output(&thread.process.mem, set, &pending.to_ne_bytes()[..size as usize])
}
