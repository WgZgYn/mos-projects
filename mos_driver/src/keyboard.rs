use conquer_once::spin::OnceCell;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use log::{error, warn};
use mos_task::scheduler::{ParkDecision, park_current};
use pc_keyboard::{
    DecodedKey, HandleControl, Keyboard, KeyboardLayout, ScancodeSet, ScancodeSet1, layouts,
};

static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// 被中断处理程序调用
/// 不能阻塞或者分配
pub fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCANCODE_QUEUE.try_get()
        && queue.push(scancode).is_ok()
    {
        WAKER.wake(); // new
    }
}

pub fn try_init_once_scancode_queue() {
    match SCANCODE_QUEUE.try_init_once(|| ArrayQueue::new(100)) {
        Ok(_) => (),
        Err(conquer_once::TryInitError::AlreadyInit) => {
            static WARN_ONCE: AtomicBool = AtomicBool::new(false);
            if !WARN_ONCE.swap(true, Ordering::AcqRel) {
                warn!("SCANCODE_QUEUE is already initialized");
            }
        }
        Err(conquer_once::TryInitError::WouldBlock) => {
            error!("SCANCODE_QUEUE initialization twice");
        }
    }
}

pub fn has_key_code() -> bool {
    SCANCODE_QUEUE
        .try_get()
        .map(|q| !q.is_empty())
        .unwrap_or(false)
}

pub fn try_get_key_code() -> Option<u8> {
    SCANCODE_QUEUE.try_get().ok().and_then(|q| q.pop())
}

pub struct ScancodeSyncStream {
    _private: (),
}

impl Default for ScancodeSyncStream {
    fn default() -> Self {
        Self::new()
    }
}

impl ScancodeSyncStream {
    pub fn new() -> Self {
        try_init_once_scancode_queue();
        Self { _private: () }
    }
    pub fn next_scancode(&mut self) -> u8 {
        loop {
            if let Some(code) = try_get_key_code() {
                return code;
            }
            park_current(|task| {
                if has_key_code() {
                    ParkDecision::Unpark
                } else {
                    WAKER.register(&Waker::from(task.clone()));
                    ParkDecision::Park(())
                }
            })
        }
    }
    pub fn next_scancode_nonblocking(&mut self) -> Option<u8> {
        try_get_key_code()
    }
}

pub struct CharSyncStream<L, S>
where
    S: ScancodeSet,
    L: KeyboardLayout,
{
    keyboard: Keyboard<L, S>,
    stream: ScancodeSyncStream,
}

impl<L, S> CharSyncStream<L, S>
where
    S: ScancodeSet,
    L: KeyboardLayout,
{
    pub fn new(scancode_set: S, layout: L) -> Self {
        Self {
            keyboard: Keyboard::new(scancode_set, layout, HandleControl::Ignore),
            stream: ScancodeSyncStream::new(),
        }
    }
    pub fn next_char(&mut self) -> char {
        loop {
            let scancode = self.stream.next_scancode();
            if let Ok(Some(key_event)) = self.keyboard.add_byte(scancode)
                && let Some(key) = self.keyboard.process_keyevent(key_event)
                && let DecodedKey::Unicode(character) = key
            {
                return character;
            }
        }
    }
    pub fn next_char_nonblocking(&mut self) -> Option<char> {
        let scancode = self.stream.next_scancode_nonblocking()?;
        if let Ok(Some(key_event)) = self.keyboard.add_byte(scancode)
            && let Some(key) = self.keyboard.process_keyevent(key_event)
            && let DecodedKey::Unicode(character) = key
        {
            return Some(character);
        }
        None
    }
}

impl Default for CharSyncStream<layouts::Us104Key, ScancodeSet1> {
    fn default() -> Self {
        Self::new(ScancodeSet1::new(), layouts::Us104Key)
    }
}
