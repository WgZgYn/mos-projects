//! DisplayManager：窗口列表、脏区域收集、合成渲染

use crate::Rect;
use crate::traits::{Compositor, PixelBuffer, PixelSink, PixelWindow};
use crate::window::{Surface, Window};
use alloc::sync::Arc;
use alloc::vec::Vec;
use mos_arch::sync::SpinIrqLock;
use spin::Mutex;

pub struct DisplayManager<W: PixelWindow> {
    windows: Vec<Arc<Mutex<W>>>,
    extra_dirty: Vec<Rect>,
}

impl<W: PixelWindow> Compositor for DisplayManager<W> {
    fn flush(&mut self, sink: &mut impl PixelSink) {
        self.flush_with_damage(sink);
    }
}

impl<W: PixelWindow> DisplayManager<W> {
    pub fn flush_with_damage(&mut self, sink: &mut impl PixelSink) -> Vec<Rect> {
        let mut dirty = self.collect_dirty();
        dirty.append(&mut self.extra_dirty);
        for dirty in dirty.iter().copied() {
            // A removed window may expose desktop with no kernel window beneath it.
            let screen = Rect {
                x: 0,
                y: 0,
                w: sink.width(),
                h: sink.height(),
            };
            let Some(dirty) = dirty.intersect(screen) else {
                continue;
            };
            let black = [crate::RGBA {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            }; 128];
            for y in dirty.y..dirty.y + dirty.h {
                let mut x = dirty.x;
                while x < dirty.x + dirty.w {
                    let n = (dirty.x + dirty.w - x).min(black.len());
                    sink.blit_row(x, y, &black[..n]);
                    x += n;
                }
            }
            sink.restore_background(dirty);
            for window in self.windows.iter() {
                let Some(window) = window.try_lock() else {
                    continue;
                };
                let bound = window.bound();
                if let Some(Rect { x, y, w, h }) = bound.intersect(dirty) {
                    let buffer = window.buffer();
                    for y in y..y + h {
                        let sy = y - window.y_start();
                        let sx = x - window.x_start();
                        sink.blit_row(x, y, &buffer.row(sy).unwrap()[sx..sx + w]);
                    }
                }
            }
        }
        dirty
    }
}

impl<W: PixelWindow> DisplayManager<W> {
    pub fn collect_dirty(&mut self) -> Vec<Rect> {
        let mut dirty = Vec::new();
        for window in self.windows.iter() {
            let Some(mut w) = window.try_lock() else {
                continue;
            };
            for rect in w.take_dirty() {
                dirty.push(rect);
            }
        }
        dirty
    }

    pub fn add_extra_dirty(&mut self, rect: Rect) {
        // Keep distant damage separate: merging a mouse cursor near the top
        // with a terminal cursor on the last row repaints most of the display
        // for every key. Bound the queue without making that the common case.
        let overlap = self
            .extra_dirty
            .iter()
            .position(|pending| pending.intersect(rect).is_some());
        let index = overlap.or_else(|| (self.extra_dirty.len() >= 32).then_some(0));
        if let Some(index) = index {
            let pending = &mut self.extra_dirty[index];
            let right = pending
                .x
                .saturating_add(pending.w)
                .max(rect.x.saturating_add(rect.w));
            let bottom = pending
                .y
                .saturating_add(pending.h)
                .max(rect.y.saturating_add(rect.h));
            pending.x = pending.x.min(rect.x);
            pending.y = pending.y.min(rect.y);
            pending.w = right - pending.x;
            pending.h = bottom - pending.y;
        } else {
            self.extra_dirty.push(rect);
        }
    }
}

impl DisplayManager<Window<Surface>> {
    pub fn add_new_window(&mut self, info: Rect) -> Arc<Mutex<Window<Surface>>> {
        self.windows
            .push_mut(Arc::new(Mutex::new(Window::new(
                info.x,
                info.y,
                Surface::new(info.w, info.h),
            ))))
            .clone()
    }
}

impl<W: PixelWindow> DisplayManager<W> {
    pub const fn new() -> Self {
        Self {
            windows: Vec::new(),
            extra_dirty: Vec::new(),
        }
    }
}

pub static DISPLAY_MANAGER: SpinIrqLock<DisplayManager<Window<Surface>>> =
    SpinIrqLock::new(DisplayManager::new());
