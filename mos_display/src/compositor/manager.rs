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
        let mut dirty = self.collect_dirty();
        dirty.append(&mut self.extra_dirty);
        for dirty in dirty {
            for window in self.windows.iter() {
                let Some(window) = window.try_lock() else { continue };
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
    }
}

impl<W: PixelWindow> DisplayManager<W> {
    pub fn collect_dirty(&mut self) -> Vec<Rect> {
        let mut dirty = Vec::new();
        for window in self.windows.iter() {
            let Some(mut w) = window.try_lock() else { continue };
            for rect in w.take_dirty() { dirty.push(rect); }
        }
        dirty
    }

    pub fn add_extra_dirty(&mut self, rect: Rect) { self.extra_dirty.push(rect); }
}

impl DisplayManager<Window<Surface>> {
    pub fn add_new_window(&mut self, info: Rect) -> Arc<Mutex<Window<Surface>>> {
        self.windows
            .push_mut(Arc::new(Mutex::new(Window::new(info.x, info.y, Surface::new(info.w, info.h)))))
            .clone()
    }
}

impl<W: PixelWindow> DisplayManager<W> {
    pub const fn new() -> Self { Self { windows: Vec::new(), extra_dirty: Vec::new() } }
}

pub static DISPLAY_MANAGER: SpinIrqLock<DisplayManager<Window<Surface>>> =
    SpinIrqLock::new(DisplayManager::new());
