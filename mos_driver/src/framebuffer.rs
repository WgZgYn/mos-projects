use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use conquer_once::spin::OnceCell;
use core::fmt;
use mos_arch::with_disabled_interrupts;
use mos_lib::sync::DummyLock;
use noto_sans_mono_bitmap::{
    FontWeight, RasterHeight, RasterizedChar, get_raster, get_raster_width,
};
// TODO:
// DOUBLE BUFFERING
// PAGE FLIP

/// Additional vertical space between lines
const LINE_SPACING: usize = 2;
/// Additional horizontal space between characters.
const LETTER_SPACING: usize = 1;

/// Padding from the border. Prevent that font is too close to border.
const BORDER_PADDING: usize = 4;

/// Constants for the usage of the [`noto_sans_mono_bitmap`] crate.
mod font_constants {
    use super::*;

    /// Height of each char raster. The font size is ~0.84% of this. Thus, this is the line height that
    /// enables multiple characters to be side-by-side and appear optically in one line in a natural way.
    pub const CHAR_RASTER_HEIGHT: RasterHeight = RasterHeight::Size16;

    /// The width of each single symbol of the mono space font.
    pub const CHAR_RASTER_WIDTH: usize = get_raster_width(FontWeight::Regular, CHAR_RASTER_HEIGHT);

    /// Backup character if a desired symbol is not available by the font.
    /// The '�' character requires the feature "unicode-specials".
    pub const BACKUP_CHAR: char = '�';

    pub const FONT_WEIGHT: FontWeight = FontWeight::Regular;
}

/// Returns the raster of the given char or the raster of [`font_constants::BACKUP_CHAR`].
/// Another impl use '?' as the backup char
fn get_char_raster_safe(c: char) -> RasterizedChar {
    use font_constants::*;
    let fallback = || {
        get_raster(BACKUP_CHAR, FONT_WEIGHT, CHAR_RASTER_HEIGHT).unwrap_or_else(|| {
            get_raster('?', FONT_WEIGHT, CHAR_RASTER_HEIGHT).expect("Backup raster must exist")
        })
    };

    if !c.is_ascii_graphic() && c != ' ' {
        // 不可打印字符
        return fallback();
    }

    get_raster(c, FONT_WEIGHT, CHAR_RASTER_HEIGHT).unwrap_or_else(fallback)
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
}

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
        };
        logger.clear();
        logger
    }

    pub fn info(&self) -> FrameBufferInfo {
        self.info
    }

    pub fn get_raw_buffer(&mut self) -> &mut [u8] {
        self.framebuffer
    }

    fn newline(&mut self) {
        self.y_pos += font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        self.carriage_return()
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    /// Erases all text on the screen. Resets `self.x_pos` and `self.y_pos`.
    pub fn clear(&mut self) {
        self.x_pos = BORDER_PADDING;
        self.y_pos = BORDER_PADDING;
        self.framebuffer.fill(0);
    }

    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    /// Writes a single char to the framebuffer. Takes care of special control characters, such as
    /// newlines and carriage returns.
    fn write_char(&mut self, c: char) {
        match c {
            '\n' => self.newline(),
            '\r' => self.carriage_return(),
            '\t' => self.render_char(' '),
            c if c < ' ' => (), // 忽略控制字符
            c => self.render_char(c),
        }
    }

    fn render_char(&mut self, c: char) {
        let rendered = get_char_raster_safe(c);
        if self.x_pos + rendered.width() >= self.width() {
            self.newline();
        }
        if self.y_pos + rendered.height() >= self.height() {
            self.clear()
        }
        self.write_rendered_char(rendered);
    }

    /// Prints a rendered char into the framebuffer.
    /// Updates `self.x_pos`.
    fn write_rendered_char(&mut self, rendered_char: RasterizedChar) {
        for (y, row) in rendered_char.raster().iter().enumerate() {
            for (x, byte) in row.iter().enumerate() {
                self.write_pixel(self.x_pos + x, self.y_pos + y, *byte);
            }
        }
        self.x_pos += rendered_char.width() + LETTER_SPACING;
    }

    // fn get_front_color(&self, intensity: u8) -> [u8; 4] {
    //     match self.info.pixel_format {
    //         PixelFormat::Rgb => [intensity, intensity, (intensity as f32 * 0.8) as u8, 0],
    //         PixelFormat::Bgr => [(intensity as f32 * 0.8) as u8, intensity, intensity, 0],
    //         PixelFormat::U8 => [if intensity > 128 { 0xF } else { 0x0 }, 0, 0, 0],
    //         _ => [0, 0, 0, 0],
    //     }
    // }

    fn get_front_color(&self, intensity: u8) -> [u8; 4] {
        match self.info.pixel_format {
            // Rgb格式：更改为蓝色主题
            PixelFormat::Rgb => [
                intensity,
                (intensity as f32 * 0.6) as u8,
                (intensity as f32 * 0.9) as u8,
                255,
            ],
            // Bgr格式：更改为绿色主题
            PixelFormat::Bgr => [
                (intensity as f32 * 0.5) as u8,
                intensity,
                (intensity as f32 * 0.7) as u8,
                255,
            ],
            // U8格式：更改为彩色显示
            PixelFormat::U8 => [
                if intensity > 192 { 255 } else { 0 },  // 红色分量
                if intensity > 128 { 200 } else { 50 }, // 绿色分量
                if intensity > 64 { 150 } else { 100 }, // 蓝色分量
                255,
            ],
            _ => [0, 0, 0, 255], // 不透明黑色作为默认
        }
    }

    fn write_pixel(&mut self, x: usize, y: usize, intensity: u8) {
        if x >= self.width() || y >= self.height() {
            return; // 防止越界写入
        }

        let pixel_offset = y * self.info.stride + x;
        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        // 美化颜色：让文字更亮，背景更柔和
        let fg_color = self.get_front_color(intensity);
        self.framebuffer[byte_offset..byte_offset + bytes_per_pixel]
            .copy_from_slice(&fg_color[..bytes_per_pixel]);
    }
}

unsafe impl Send for FrameBufferWriter {}
unsafe impl Sync for FrameBufferWriter {}

impl fmt::Write for FrameBufferWriter {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            self.write_char(c);
        }
        Ok(())
    }
}

/// because the buffer is used as logger,
/// the logger is without interrupt
/// so here use no lock
pub static FRAME_BUFFER: OnceCell<DummyLock<FrameBufferWriter>> = OnceCell::uninit();
pub fn init(fb: &'static mut FrameBuffer) -> FrameBufferInfo {
    let info = fb.info();
    let bf = fb.buffer_mut();
    FRAME_BUFFER.init_once(|| DummyLock::new(FrameBufferWriter::new(bf, info)));
    info
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    with_disabled_interrupts(|| {
        if let Ok(f) = FRAME_BUFFER.try_get() {
            unsafe {
                f.access(|f| {
                    let _ = f.write_fmt(args);
                })
            }
        }
    })
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::framebuffer::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($fmt:expr) => ($crate::print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::print!(
        concat!($fmt, "\n"), $($arg)*));
}
