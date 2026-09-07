//! 用户态窗口注册表

use crate::Rect;
use crate::traits::{PixelBuffer, PixelSink, PixelWindow};
use crate::window::{SharedSurface, Window};
use alloc::sync::Arc;
use alloc::vec::Vec;
use mos_arch::sync::SpinIrqLock;
use spin::Mutex;

type SharedWindow = Arc<Mutex<Window<SharedSurface>>>;

struct WindowEntry {
    id: u64,
    pid: u64,
    window: SharedWindow,
}

static NEXT_WINDOW_ID: SpinIrqLock<u64> = SpinIrqLock::new(0);
static USER_WINDOWS: SpinIrqLock<Vec<WindowEntry>> = SpinIrqLock::new(Vec::new());

pub fn register(window: SharedWindow, pid: u64) -> u64 {
    let mut list = USER_WINDOWS.lock();
    let id = {
        let mut id_gen = NEXT_WINDOW_ID.lock();
        let id = *id_gen;
        *id_gen += 1;
        id
    };
    list.push(WindowEntry { id, pid, window });
    id
}

pub fn mark_dirty(window_id: u64, x: usize, y: usize, w: usize, h: usize) -> bool {
    let list = USER_WINDOWS.lock();
    if let Some(entry) = list.iter().find(|e| e.id == window_id) {
        if let Some(mut win) = entry.window.try_lock() {
            win.add_dirty(Rect { x, y, w, h });
            return true;
        }
    }
    false
}

pub fn destroy(window_id: u64) {
    USER_WINDOWS.lock().retain(|e| e.id != window_id);
}

pub fn flush(sink: &mut impl PixelSink) {
    let mut list = USER_WINDOWS.lock();
    let mut dirty = Vec::new();
    for entry in list.iter() {
        if let Some(mut win) = entry.window.try_lock() {
            for rect in win.take_dirty() { dirty.push(rect); }
        }
    }
    for dirty in dirty {
        for entry in list.iter() {
            let Some(win) = entry.window.try_lock() else { continue };
            let bound = win.bound();
            if let Some(Rect { x, y, w, h }) = bound.intersect(dirty) {
                let buffer = win.buffer();
                for row in y..y + h {
                    let sy = row - win.y_start();
                    let sx = x - win.x_start();
                    sink.blit_row(x, row, &buffer.row(sy).unwrap()[sx..sx + w]);
                }
            }
        }
    }
}
