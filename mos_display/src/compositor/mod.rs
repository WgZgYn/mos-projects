pub mod cursor;
pub mod manager;
pub mod user;

use crate::traits::{Compositor, PixelSink};
use crate::Rect;
use crate::window::{Surface, Window};
use alloc::sync::Arc;
use spin::Mutex;

use manager::DISPLAY_MANAGER;

pub use user::{register as register_user_window, mark_dirty as mark_window_dirty,
               destroy as destroy_user_window};

pub fn render_to(sink: &mut impl PixelSink) {
    // 擦除旧光标（先取出旧位置，尽快释放锁）
    let old_rect = cursor::CURSOR.lock().take_prev_rect();
    if let Some(old) = old_rect {
        DISPLAY_MANAGER.lock().add_extra_dirty(old);
    }

    // 绘制所有窗口
    DISPLAY_MANAGER.lock().flush(sink);
    user::flush(sink);

    // 叠加新光标
    let mut c = cursor::CURSOR.lock();
    if c.visible {
        let rect = Rect { x: c.x, y: c.y, w: 13, h: 20 };
        c.draw(sink);
        c.save_rect(rect);
    }
}

pub fn new_window(info: Rect) -> Arc<Mutex<Window<Surface>>> {
    DISPLAY_MANAGER.lock().add_new_window(info)
}

pub fn update_cursor(x: usize, y: usize) {
    let mut c = cursor::CURSOR.lock();
    c.x = x;
    c.y = y;
}

pub fn cursor_position() -> (usize, usize) {
    let c = cursor::CURSOR.lock();
    (c.x, c.y)
}
