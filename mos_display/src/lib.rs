#![no_std]

extern crate alloc;

mod adapter;
mod compositor;
pub mod traits;
pub mod window;
pub mod console;

use alloc::sync::Arc;
use bootloader_api::info::{FrameBuffer, PixelFormat};
use spin::Mutex;
use crate::window::{Surface, Window};

pub type SharedWindow = Arc<Mutex<Window<Surface>>>;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RGBA {
    pub const fn from_rgba_u8(rgba: [u8; 4]) -> Self {
        Self {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        }
    }
    pub fn blend(front: RGBA, back: RGBA, alpha: u8) -> RGBA {
        let a = alpha as u16;

        let r = (a * front.r as u16 + (255 - a) * back.r as u16) / 255;
        let g = (a * front.g as u16 + (255 - a) * back.g as u16) / 255;
        let b = (a * front.b as u16 + (255 - a) * back.b as u16) / 255;

        RGBA {
            r: r as u8,
            g: g as u8,
            b: b as u8,
            a: 255,
        }
    }
    pub fn into_target(self, pixel_format: PixelFormat) -> [u8; 4] {
        match pixel_format {
            PixelFormat::Rgb => [self.r, self.g, self.b, self.a],
            PixelFormat::Bgr => [self.b, self.g, self.r, self.a],
            _ => [0; 4],
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub w: usize,
    pub h: usize,
}

impl Rect {
    pub const fn intersect(self, rhs: Self) -> Option<Self> {
        let left = if self.x > rhs.x { self.x } else { rhs.x };
        let right = if self.x + self.w < rhs.x + rhs.w {
            self.x + self.w
        } else {
            rhs.x + rhs.w
        };
        let top = if self.y > rhs.y { self.y } else { rhs.y };
        let bottom = if self.y + self.h < rhs.y + rhs.h {
            self.y + self.h
        } else {
            rhs.y + rhs.h
        };

        if left < right && top < bottom {
            Some(Self {
                x: left,
                y: top,
                w: right - left,
                h: bottom - top,
            })
        } else {
            None
        }
    }
}

pub fn init(framebuffer: &'static mut FrameBuffer) {
    adapter::init_framebuffer_adapter(framebuffer);
}

pub fn render_to_framebuffer() {
    adapter::access_framebuffer_adapter(|adapter| {
        compositor::render_to(adapter);
    });
}

pub fn new_window(info: Rect) -> SharedWindow {
    compositor::new_window(info)
}

// ─── 用户态窗口 API (供 mos_syscall 调用) ──────────

use crate::window::SharedSurface;

pub fn register_user_window(
    window: Arc<Mutex<Window<SharedSurface>>>,
    pid: u64,
) -> u64 {
    compositor::register_user_window(window, pid)
}

pub fn mark_user_window_dirty(window_id: u64, x: usize, y: usize, w: usize, h: usize) -> bool {
    compositor::mark_window_dirty(window_id, x, y, w, h)
}

pub fn destroy_user_window(window_id: u64) {
    compositor::destroy_user_window(window_id)
}

// ─── 光标 API ──────────────────────────────────────

pub fn update_cursor(x: usize, y: usize) {
    compositor::update_cursor(x, y);
}

pub fn cursor_position() -> (usize, usize) {
    compositor::cursor_position()
}
