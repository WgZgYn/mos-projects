use crate::traits::PixelBuffer;
use alloc::sync::Arc;
use alloc::vec::Vec;
use mos_arch::sync::SpinIrqLock;
use spin::Mutex;
use crate::Rect;
use crate::traits::{Compositor, PixelSink, PixelWindow};
use crate::window::{Surface, Window};

pub struct DisplayManager<Window: PixelWindow> {
    windows: Vec<Arc<Mutex<Window>>>,
}

impl<Window: PixelWindow> Compositor for DisplayManager<Window> {
    fn flush(&mut self, sink: &mut impl PixelSink) {
        let dirty = self.collect_dirty();
        for dirty in dirty {
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
    }
}

impl<Window: PixelWindow> DisplayManager<Window> {
    pub fn collect_dirty(&mut self) -> Vec<Rect> {
        let mut dirty = Vec::new();
        for window in self.windows.iter() {
            let Some(mut window) = window.try_lock() else {
                continue;
            };
            for rect in window.take_dirty() {
                dirty.push(rect);
            }
        }
        dirty
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

impl<Window: PixelWindow> DisplayManager<Window> {
    const fn new() -> Self {
        Self {
            windows: Vec::new(),
        }
    }
}

static DISPLAY_MANAGER: SpinIrqLock<DisplayManager<Window<Surface>>> =
    SpinIrqLock::new(DisplayManager::new());

pub fn render_to(sink: &mut impl PixelSink) {
    DISPLAY_MANAGER.access(|d| d.flush(sink));
}

pub fn new_window(info: Rect) -> Arc<Mutex<Window<Surface>>> {
    DISPLAY_MANAGER.access(|d| d.add_new_window(info))
}
