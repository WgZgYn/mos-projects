use crate::traits::PixelBuffer;
use crate::traits::PixelWindow;
use crate::{render_to_framebuffer, Rect, SharedWindow, RGBA};
use core::fmt;
use core::fmt::Write;
use core::ops::DerefMut;
use log::{Log, Metadata, Record};
use mos_task::preempt_session::PreemptSpinLock;
use noto_sans_mono_bitmap::{get_raster, FontWeight, RasterHeight, RasterizedChar};

/// Additional vertical space between lines
const LINE_SPACING: usize = 2;
/// Additional horizontal space between characters.
const LETTER_SPACING: usize = 1;

/// Padding from the border. Prevent that font is too close to border.
const BORDER_PADDING: usize = 4;

const FONT_WEIGHT: FontWeight = FontWeight::Regular;
const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;
const BACKUP_CHAR_RASTER: RasterizedChar = get_char_raster('�').unwrap();
const fn get_char_raster(c: char) -> Option<RasterizedChar> {
    get_raster(c, FONT_WEIGHT, CHAR_RASTER_HEIGHT)
}

pub struct Console {
    window: SharedWindow,
    width: usize,
    height: usize,
    x_pos: usize,
    y_pos: usize,
    ft_color: RGBA,
    bg_color: RGBA,
}

impl Console {
    pub fn new(window: SharedWindow) -> Self {
        let guard = window.lock();
        let width = guard.buffer().width();
        let height = guard.buffer().height();
        drop(guard);
        let mut ret = Self {
            window,
            width,
            height,
            x_pos: 0,
            y_pos: 0,
            ft_color: RGBA {
                r: 220,
                g: 220,
                b: 220,
                a: 255,
            },
            bg_color: RGBA {
                r: 30,
                g: 30,
                b: 30,
                a: 255,
            },
        };
        ret.clear();
        ret
    }

    fn write_raster_pixel(&mut self, x_pos: usize, y_pos: usize, alpha: u8) {
        let color = RGBA::blend(self.ft_color, self.bg_color, alpha);
        let mut guard = self.window.lock();
        let buffer = guard.buffer_mut();
        buffer.row_mut(y_pos).unwrap()[x_pos] = color;
    }
    fn render_char_raster(&mut self, c: RasterizedChar) {
        for (y, row) in c.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_raster_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += c.width() + LETTER_SPACING;
    }
    fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;

        let mut guard = self.window.lock();
        let buffer = guard.buffer_mut();
        let h = buffer.height();
        for y in 0..h {
            buffer.row_mut(y).unwrap().fill(self.bg_color);
        }
    }
    fn newline(&mut self) {
        self.y_pos += CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return();
        if self.y_pos >= self.height {
            self.clear()
        }
    }
    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }
    fn render_char(&mut self, c: char) {
        let pox_x = self.x_pos;
        let pox_y = self.y_pos;
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            c => {
                if let Some(r) = get_char_raster(c) {
                    if self.x_pos + r.width() >= self.width {
                        self.newline();
                    }
                    if self.y_pos + r.height() >= self.height {
                        self.clear()
                    }
                    self.render_char_raster(r);
                }
            }
        }
        if self.y_pos < pox_y {
            self.window.lock().add_dirty(Rect {
                x: 0,
                y: 0,
                w: self.width,
                h: self.height,
            })
        } else if self.y_pos == pox_y {
            self.window.lock().add_dirty(Rect {
                x: pox_x.min(self.x_pos),
                y: pox_y,
                w: pox_x.max(self.x_pos),
                h: 1,
            })
        } else {
            self.window.lock().add_dirty(Rect {
                x: 0,
                y: pox_y,
                w: self.width,
                h: self.y_pos - pox_y,
            })
        }
    }
    pub fn commit(&mut self) {
        render_to_framebuffer();
    }
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.render_char(c);
        }
        Ok(())
    }
}

pub struct ConsoleLogger(PreemptSpinLock<Console>);

impl ConsoleLogger {
    pub const fn new(console: Console) -> Self {
        Self(PreemptSpinLock::new(console))
    }
}

impl Log for ConsoleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= log::Level::Debug
    }

    fn log(&self, record: &Record) {
        let mut guard = self.0.lock();
        let _ = writeln!(guard.deref_mut(), "{}", record.args());
        guard.commit();
    }

    fn flush(&self) {
        self.0.lock().commit()
    }
}
