#![allow(unused)]

use alloc::vec::Vec;
use core::iter::Iterator;
use core::sync::atomic::{AtomicU64, Ordering};
use core::task::Waker;
use mos_arch::sync::{IrqLocked, SpinIrqLock};

pub struct SystemTimer {
    counter: AtomicU64,
}

impl SystemTimer {
    const fn new() -> SystemTimer {
        SystemTimer {
            counter: AtomicU64::new(0),
        }
    }
    pub fn tick(&self) -> u64 {
        self.counter.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn get_tick(&self) -> u64 {
        self.counter.load(Ordering::Relaxed)
    }
}

static SYSTEM_TIMER: SystemTimer = SystemTimer::new();
static TIMEWHEEL: SpinIrqLock<TimeWheel> = SpinIrqLock::new(TimeWheel::new());

/// # Safety
/// called by interrupt context
pub unsafe fn tick() -> u64 {
    let tick = SYSTEM_TIMER.tick();
    TIMEWHEEL.access(|v| v.tick(tick));
    tick
}

pub fn get_tick() -> u64 {
    SYSTEM_TIMER.get_tick()
}

#[derive(Default)]
pub struct TimerEntry {
    expire_tick: u64,
    waker: Option<Waker>,
}

impl TimerEntry {
    pub fn new(expire_tick: u64, waker: Option<Waker>) -> TimerEntry {
        TimerEntry { expire_tick, waker }
    }
}

pub struct Wheel<const N: usize> {
    slot: [Vec<TimerEntry>; N],
    idx: usize,
}
impl<const N: usize> Wheel<N> {
    const fn new() -> Wheel<N> {
        Self {
            slot: [const { Vec::new() }; N],
            idx: 0,
        }
    }
    fn tick(&mut self) -> Vec<TimerEntry> {
        let mut reinsert = Vec::new();
        core::mem::swap(&mut reinsert, &mut self.slot[self.idx]);
        self.idx = (self.idx + 1) % N;
        reinsert
    }
}

const fn slot_interval() -> [u64; 10] {
    let mut slot: [u64; 10] = [0; 10];
    slot[0] = 1;
    let mut i = 1;
    while i < 10 {
        slot[i] = slot[i - 1] * 10;
        i += 1;
    }
    slot
}

const SLOTS_TICK_INTERVAL_MS: [u64; 10] = slot_interval();

struct TimeWheel {
    slots: [Wheel<10>; 10],
}

impl TimeWheel {
    const fn new() -> TimeWheel {
        TimeWheel {
            slots: [const { Wheel::new() }; 10],
        }
    }

    // TODO: support cancel the task
    fn add_timer(&mut self, entry: TimerEntry, current_tick: u64) {
        let (target_slot_idx, slot_offset_idx) = SLOTS_TICK_INTERVAL_MS
            .into_iter()
            .enumerate()
            .rfind(|&(_, v)| entry.expire_tick > v + current_tick)
            .map(|(i, v)| (i, ((entry.expire_tick - current_tick - v) / v) as usize))
            .unwrap_or((0, 0));
        const N: usize = 10;
        let mut offset = (slot_offset_idx + self.slots[target_slot_idx].idx) % 10;
        self.slots[target_slot_idx].slot[offset].push(entry);
    }

    fn tick(&mut self, current_tick: u64) {
        let reinsert = SLOTS_TICK_INTERVAL_MS
            .into_iter()
            .enumerate()
            .filter(|(_, v)| current_tick.is_multiple_of(*v))
            .flat_map(|(idx, _)| self.slots[idx].tick())
            .filter(|entry| {
                let reinsert = entry.expire_tick > current_tick;
                if !reinsert {
                    entry.waker.as_ref().inspect(|v| v.wake_by_ref());
                }
                reinsert
            })
            .collect::<Vec<_>>(); // TODO: avoid use vec collect

        reinsert
            .into_iter()
            .for_each(|entry| self.add_timer(entry, current_tick));
    }
}

pub fn add_timer(expire_tick: u64, waker: Option<Waker>) {
    let tick = get_tick();
    if tick >= expire_tick {
        return;
    }
    TIMEWHEEL.access(|v| {
        v.add_timer(TimerEntry::new(expire_tick, waker), tick);
    })
}


pub fn init_timer() {

}