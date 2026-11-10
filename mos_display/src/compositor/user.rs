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
    bound: Rect,
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
    let bound = window.lock().bound();
    window.lock().add_dirty(Rect { x: 0, y: 0, w: bound.w, h: bound.h });
    list.push(WindowEntry { id, pid, window, bound });
    id
}

pub fn mark_dirty(window_id: u64, pid: u64, x: usize, y: usize, w: usize, h: usize) -> bool {
    let list = USER_WINDOWS.lock();
    if let Some(entry) = list.iter().find(|e| e.id == window_id && e.pid == pid) {
        entry.window.lock().add_dirty(Rect { x, y, w, h });
        return true;
    }
    false
}

pub fn destroy(window_id: u64, pid: u64) -> bool {
    let mut list = USER_WINDOWS.lock();
    let Some(index) = list.iter().position(|e| e.id == window_id && e.pid == pid)
        else { return false; };
    let removed = list.remove(index);
    // Repaint the exposed background and surviving windows in stacking order.
    super::manager::DISPLAY_MANAGER.lock().add_extra_dirty(removed.bound);
    true
}

pub fn flush(sink: &mut impl PixelSink, mut dirty: Vec<Rect>) {
    let list = USER_WINDOWS.lock();
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
