//! 光标：形状定义、位置跟踪、渲染

use crate::RGBA;
use crate::Rect;
use crate::traits::PixelSink;
use mos_arch::sync::SpinIrqLock;

const CURSOR_W: usize = 11;
const CURSOR_H: usize = 18;
const CURSOR_BITMAP: [u32; CURSOR_H] = [
    0b10000000_000, 0b11000000_000, 0b11100000_000,
    0b11110000_000, 0b11111000_000, 0b11111100_000,
    0b11111110_000, 0b11111111_000, 0b11111100_000,
    0b11001100_000, 0b10001110_000, 0b00001110_000,
    0b00000110_000, 0b00000111_000, 0b00000111_000,
    0b00000011_000, 0b00000011_000, 0b00000001_000,
];

pub struct Cursor {
    pub x: usize,
    pub y: usize,
    pub visible: bool,
    prev_rect: Option<Rect>,
}

impl Cursor {
    pub const fn new() -> Self {
        Self { x: 300, y: 300, visible: true, prev_rect: None }
    }

    pub fn take_prev_rect(&mut self) -> Option<Rect> { self.prev_rect.take() }
    pub fn save_rect(&mut self, rect: Rect) { self.prev_rect = Some(rect); }

    pub fn draw(&self, sink: &mut impl PixelSink) {
        let white = RGBA { r: 255, g: 255, b: 255, a: 255 };
        let black = RGBA { r: 0, g: 0, b: 0, a: 255 };
        let transparent = RGBA { r: 0, g: 0, b: 0, a: 0 };
        let x0 = self.x;
        let mut buf = [transparent; CURSOR_W + 1];

        for (dy, row_mask) in CURSOR_BITMAP.iter().enumerate() {
            buf.fill(transparent);
            for dx in 0..CURSOR_W {
                if (*row_mask >> (CURSOR_W - 1 - dx)) & 1 != 0 {
                    if dy > 0 && dx > 0 { buf[dx - 1] = black; }
                    buf[dx] = white;
                }
            }
            sink.blit_row(x0.saturating_sub(1), self.y + dy, &buf[..CURSOR_W]);
        }
    }
}

pub static CURSOR: SpinIrqLock<Cursor> = SpinIrqLock::new(Cursor::new());
