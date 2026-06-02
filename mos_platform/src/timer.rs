use crate::acpi::{pm_timer_freq, pm_timer_mask, pm_timer_tick};
use alloc::vec::Vec;
use core::sync::atomic::Ordering;
use core::task::Waker;
use mos_arch::percpu::{current_raw, try_current_raw};
use mos_arch::sync::SpinIrqLock;

#[derive(Default)]
struct MonotonicClock {
    total_pm_ticks: u64,
    last_raw_tick: u32,
    initialized: bool,
}

impl MonotonicClock {
    const fn new() -> Self {
        Self {
            total_pm_ticks: 0,
            last_raw_tick: 0,
            initialized: false,
        }
    }

    fn init(&mut self, raw_tick: u32) {
        self.total_pm_ticks = 0;
        self.last_raw_tick = raw_tick;
        self.initialized = true;
    }

    fn sample(&mut self, raw_tick: u32) -> u64 {
        if !self.initialized {
            self.init(raw_tick);
            return 0;
        }

        let delta = raw_tick.wrapping_sub(self.last_raw_tick) & pm_timer_mask();
        self.last_raw_tick = raw_tick;
        self.total_pm_ticks += delta as u64;
        self.total_pm_ticks
    }
}

struct TimerEntry {
    deadline: u64,
    waker: Waker,
}

struct TimerQueue {
    entries: Vec<TimerEntry>,
}

impl TimerQueue {
    const fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add_timer(&mut self, deadline: u64, waker: Waker) {
        self.entries.push(TimerEntry { deadline, waker });
    }

    fn wake_expired(&mut self, now: u64) -> Vec<Waker> {
        let mut wakers = Vec::new();
        let mut idx = 0;
        while idx < self.entries.len() {
            if self.entries[idx].deadline <= now {
                let entry = self.entries.swap_remove(idx);
                wakers.push(entry.waker);
            } else {
                idx += 1;
            }
        }
        wakers
    }
}

static MONOTONIC_CLOCK: SpinIrqLock<MonotonicClock> = SpinIrqLock::new(MonotonicClock::new());
static TIMER_QUEUE: SpinIrqLock<TimerQueue> = SpinIrqLock::new(TimerQueue::new());

#[inline]
fn pm_ticks_per_ms() -> u64 {
    pm_timer_freq() as u64
}

#[inline]
fn ms_to_pm_ticks(ms: u64) -> u64 {
    ms.saturating_mul(pm_ticks_per_ms()).div_ceil(1000)
}

#[inline]
fn pm_ticks_to_ms(ticks: u64) -> u64 {
    ticks.saturating_mul(1000) / pm_ticks_per_ms()
}

pub fn init_monotonic_clock() {
    MONOTONIC_CLOCK.access(|clock| {
        let now = pm_timer_tick();
        clock.init(now);
    });
}

pub fn monotonic_ticks() -> u64 {
    MONOTONIC_CLOCK.access(|clock| {
        let now = pm_timer_tick();
        clock.sample(now)
    })
}

pub fn monotonic_time_ms() -> u64 {
    pm_ticks_to_ms(monotonic_ticks())
}

pub fn monotonic_deadline_after_ms(ms: u64) -> u64 {
    monotonic_ticks().saturating_add(ms_to_pm_ticks(ms))
}

pub fn deadline_expired(deadline: u64) -> bool {
    monotonic_ticks() >= deadline
}

pub fn add_timer(deadline: u64, waker: Option<Waker>) -> bool {
    if deadline_expired(deadline) {
        return false;
    }
    if let Some(waker) = waker {
        TIMER_QUEUE.access(|queue| queue.add_timer(deadline, waker));
        true
    } else {
        false
    }
}

/// # Safety
/// called by interrupt context
pub unsafe fn tick() -> u64 {
    let tick = unsafe { current_raw().tick.fetch_add(1, Ordering::Relaxed) + 1 };
    let now = monotonic_ticks();
    // drain expired wakers under lock, then wake them OUTSIDE the lock
    // to avoid AB-BA deadlock: TIMER_QUEUE → wake → SCHEDULER vs SCHEDULER → TIMER_QUEUE
    let wakers = TIMER_QUEUE.access(|queue| queue.wake_expired(now));
    for waker in wakers {
        waker.wake();
    }
    tick
}

pub fn get_tick() -> u64 {
    try_current_raw().map_or(0, |c| c.tick.load(Ordering::Relaxed))
}
