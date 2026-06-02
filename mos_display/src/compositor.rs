use crate::RGBA;
use crate::traits::PixelBuffer;
use crate::traits::{Compositor, PixelSink, PixelWindow};
use crate::window::{SharedSurface, Surface, Window};
use crate::Rect;
use alloc::sync::Arc;
use alloc::vec::Vec;
use mos_task::preempt_session::PreemptSpinLock;
use spin::Mutex;

/// 简单箭头光标形状 (11×18 像素, 1=白色前景, 0=透明)
const CURSOR_W: usize = 11;
const CURSOR_H: usize = 18;
const CURSOR_BITMAP: [u32; CURSOR_H] = [
    0b10000000_000,
    0b11000000_000,
    0b11100000_000,
    0b11110000_000,
    0b11111000_000,
    0b11111100_000,
    0b11111110_000,
    0b11111111_000,
    0b11111100_000,
    0b11001100_000,
    0b10001110_000,
    0b00001110_000,
    0b00000110_000,
    0b00000111_000,
    0b00000111_000,
    0b00000011_000,
    0b00000011_000,
    0b00000001_000,
];

struct Cursor {
    x: usize,
    y: usize,
    visible: bool,
    prev_rect: Option<Rect>,
}

impl Cursor {
    const fn new() -> Self {
        Self {
            x: 300,
            y: 300,
            visible: true,
            prev_rect: None,
        }
    }

    fn take_prev_rect(&mut self) -> Option<Rect> {
        self.prev_rect.take()
    }

    fn save_rect(&mut self, rect: Rect) {
        self.prev_rect = Some(rect);
    }
}

static CURSOR: PreemptSpinLock<Cursor> = PreemptSpinLock::new(Cursor::new());

pub struct DisplayManager<Window: PixelWindow> {
    windows: Vec<Arc<Mutex<Window>>>,
    extra_dirty: Vec<Rect>,
}

impl<Window: PixelWindow> Compositor for DisplayManager<Window> {
    fn flush(&mut self, sink: &mut impl PixelSink) {
        let mut dirty = self.collect_dirty();
        // 将旧的额外脏区域（如旧光标位置）也加入本次重绘
        dirty.append(&mut self.extra_dirty);
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

    pub fn add_extra_dirty(&mut self, rect: Rect) {
        self.extra_dirty.push(rect);
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
            extra_dirty: Vec::new(),
        }
    }
}

static DISPLAY_MANAGER: PreemptSpinLock<DisplayManager<Window<Surface>>> =
    PreemptSpinLock::new(DisplayManager::new());

pub fn render_to(sink: &mut impl PixelSink) {
    // 1. 标记旧光标区域为脏，以便擦除
    let mut cursor = CURSOR.lock();
    if let Some(old) = cursor.take_prev_rect() {
        DISPLAY_MANAGER.lock().add_extra_dirty(old);
    }

    // 2. 绘制所有窗口（包括擦除旧光标区域）
    DISPLAY_MANAGER.lock().flush(sink);
    flush_user_windows(sink);

    // 3. 叠加新光标
    if cursor.visible {
        draw_cursor(sink, &cursor);
        let rect = Rect {
            x: cursor.x,
            y: cursor.y,
            w: CURSOR_W + 1,
            h: CURSOR_H + 1,
        };
        cursor.save_rect(rect);
    }
}

fn draw_cursor(sink: &mut impl PixelSink, cursor: &Cursor) {
    let white = RGBA { r: 255, g: 255, b: 255, a: 255 };
    let black = RGBA { r: 0, g: 0, b: 0, a: 255 };
    let transparent = RGBA { r: 0, g: 0, b: 0, a: 0 };

    let x0 = cursor.x;
    let y0 = cursor.y;

    // 按行绘制光标，每行像素数据放入临时缓冲区
    let mut buf = [transparent; CURSOR_W + 1];

    for (dy, row_mask) in CURSOR_BITMAP.iter().enumerate() {
        buf.fill(transparent);
        for dx in 0..CURSOR_W {
            let bit = (*row_mask >> (CURSOR_W - 1 - dx)) & 1;
            if bit != 0 {
                // 黑色阴影（右下各偏1像素）
                if dy > 0 && dx > 0 {
                    buf[dx - 1] = black;
                }
                buf[dx] = white;
            }
        }
        let y = y0 + dy;
        sink.blit_row(x0.saturating_sub(1), y, &buf[..CURSOR_W]);
    }
}

pub fn new_window(info: Rect) -> Arc<Mutex<Window<Surface>>> {
    DISPLAY_MANAGER.lock().add_new_window(info)
}

pub fn update_cursor(x: usize, y: usize) {
    let mut cursor = CURSOR.lock();
    cursor.x = x;
    cursor.y = y;
}

pub fn cursor_position() -> (usize, usize) {
    let cursor = CURSOR.lock();
    (cursor.x, cursor.y)
}

// ─── 用户态窗口注册表 ──────────────────────────────

type SharedWindow = Arc<Mutex<Window<SharedSurface>>>;

struct WindowEntry {
    id: u64,
    #[allow(unused)]
    pid: u64,
    window: SharedWindow,
}

static NEXT_WINDOW_ID: PreemptSpinLock<u64> = PreemptSpinLock::new(0);
static USER_WINDOWS: PreemptSpinLock<Vec<WindowEntry>> = PreemptSpinLock::new(Vec::new());

pub fn register_user_window(window: SharedWindow, pid: u64) -> u64 {
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

pub fn mark_window_dirty(window_id: u64, x: usize, y: usize, w: usize, h: usize) -> bool {
    let list = USER_WINDOWS.lock();
    if let Some(entry) = list.iter().find(|e| e.id == window_id) {
        if let Some(mut win) = entry.window.try_lock() {
            win.add_dirty(Rect { x, y, w, h });
            return true;
        }
    }
    false
}

pub fn destroy_user_window(window_id: u64) {
    USER_WINDOWS.lock().retain(|e| e.id != window_id);
}

fn flush_user_windows(sink: &mut impl PixelSink) {
    let mut list = USER_WINDOWS.lock();
    let mut dirty = Vec::new();
    for entry in list.iter() {
        if let Some(mut win) = entry.window.try_lock() {
            for rect in win.take_dirty() {
                dirty.push(rect);
            }
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
