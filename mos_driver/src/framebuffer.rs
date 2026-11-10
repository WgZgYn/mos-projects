use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use conquer_once::spin::OnceCell;
use core::fmt;
use core::sync::atomic::{AtomicBool, Ordering};
use mos_arch::sync::SpinIrqLock;
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

/// Allocation-free early-boot escape filtering. After heap initialization,
/// mos_terminal owns UTF-8, ANSI and character state instead.
#[derive(Clone, Copy)]
enum AnsiState {
    Normal,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

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

    get_raster(c, FONT_WEIGHT, CHAR_RASTER_HEIGHT).unwrap_or_else(fallback)
}

/// Allows logging text to a pixel-based framebuffer.
pub struct FrameBufferWriter {
    framebuffer: &'static mut [u8],
    info: FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    ansi_state: AnsiState,
    terminal: Option<mos_terminal::Terminal>,
    cell_colors: Option<(mos_terminal::Color, mos_terminal::Color)>,
    damage: Option<[usize; 4]>,
}

impl FrameBufferWriter {
    /// Creates a new logger that uses the given framebuffer.
    pub fn new(framebuffer: &'static mut [u8], info: FrameBufferInfo) -> Self {
        let mut logger = Self {
            framebuffer,
            info,
            x_pos: 0,
            y_pos: 0,
            ansi_state: AnsiState::Normal,
            terminal: None,
            cell_colors: None,
            damage: None,
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
        self.carriage_return();
        self.scroll_if_needed();
    }

    fn scroll_if_needed(&mut self) {
        let line = font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        if self.y_pos + font_constants::CHAR_RASTER_HEIGHT.val() < self.height() {
            return;
        }
        let row_bytes = self.info.stride * self.info.bytes_per_pixel;
        let top = BORDER_PADDING * row_bytes;
        let end = self.info.height * row_bytes;
        let shift = line * row_bytes;
        if top + shift >= end || end > self.framebuffer.len() {
            self.clear();
            return;
        }
        self.framebuffer.copy_within(top + shift..end, top);
        self.framebuffer[end - shift..end].fill(0);
        self.y_pos = self.y_pos.saturating_sub(line).max(BORDER_PADDING);
    }

    fn carriage_return(&mut self) {
        self.x_pos = BORDER_PADDING;
    }

    /// Erases all text on the screen. Resets `self.x_pos` and `self.y_pos`.
    pub fn clear(&mut self) {
        if let Some(terminal) = self.terminal.as_mut() {
            terminal.reset();
        }
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
            '\x08' => self.backspace(),
            c if c < ' ' => (), // 忽略控制字符
            c => self.render_char(c),
        }
    }

    fn backspace(&mut self) {
        let char_width = font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
        if self.x_pos >= BORDER_PADDING + char_width {
            self.x_pos -= char_width;
            // 用背景色擦除该字符区域
            let char_height = font_constants::CHAR_RASTER_HEIGHT.val();
            for y in 0..char_height {
                for x in 0..font_constants::CHAR_RASTER_WIDTH {
                    self.write_pixel(self.x_pos + x, self.y_pos + y, 0); // 0 代表黑色背景
                }
            }
        }
    }

    fn render_char(&mut self, c: char) {
        let rendered = get_char_raster_safe(c);
        if self.x_pos + rendered.width() >= self.width() {
            self.newline();
        }
        self.scroll_if_needed();
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
        if let Some((foreground, background)) = self.cell_colors {
            let blend = |i: usize| {
                ((foreground[i] as u32 * intensity as u32
                    + background[i] as u32 * (255 - intensity as u32))
                    / 255) as u8
            };
            let rgb = [blend(0), blend(1), blend(2)];
            return match self.info.pixel_format {
                PixelFormat::Rgb => [rgb[0], rgb[1], rgb[2], 255],
                PixelFormat::Bgr => [rgb[2], rgb[1], rgb[0], 255],
                PixelFormat::U8 => [
                    ((rgb[0] as u32 * 30 + rgb[1] as u32 * 59 + rgb[2] as u32 * 11) / 100) as u8,
                    0,
                    0,
                    0,
                ],
                _ => [0, 0, 0, 255],
            };
        }
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
        if self.terminal.is_some() {
            // Kernel emergency/text output keeps conventional CRLF. TTY writes
            // call write_terminal directly with their actual termios policy.
            self.write_terminal(s.as_bytes(), true, true);
            return Ok(());
        }
        for c in s.chars() {
            match self.ansi_state {
                AnsiState::Normal => {
                    if c == '\x1b' {
                        self.ansi_state = AnsiState::Escape;
                    } else {
                        self.write_char(c);
                    }
                }
                AnsiState::Escape => match c {
                    '[' => self.ansi_state = AnsiState::Csi,
                    ']' => self.ansi_state = AnsiState::Osc,
                    // A non-CSI escape is one character long. Drop the
                    // sequence introducer and retain the visible character.
                    _ => {
                        self.ansi_state = AnsiState::Normal;
                        if c != '\x1b' {
                            self.write_char(c);
                        }
                    }
                },
                AnsiState::Csi => {
                    // CSI parameters/intermediates occupy 0x20..0x3f and
                    // the final byte occupies 0x40..0x7e. Ignore the
                    // sequence instead of drawing raw escape bytes.
                    if (c as u32) >= 0x40 && (c as u32) <= 0x7e {
                        self.ansi_state = AnsiState::Normal;
                    }
                }
                AnsiState::Osc => match c {
                    '\x07' => self.ansi_state = AnsiState::Normal,
                    '\x1b' => self.ansi_state = AnsiState::OscEscape,
                    _ => {}
                },
                AnsiState::OscEscape => {
                    self.ansi_state = if c == '\\' {
                        AnsiState::Normal
                    } else if c == '\x1b' {
                        AnsiState::OscEscape
                    } else {
                        AnsiState::Osc
                    };
                }
            }
        }
        Ok(())
    }
}

pub static FRAME_BUFFER: OnceCell<SpinIrqLock<FrameBufferWriter>> = OnceCell::uninit();
/// All normal framebuffer writers, including the compositor, take this first.
/// Lock order: display -> adapter (when composing) -> terminal model.
pub static DISPLAY_LOCK: SpinIrqLock<()> = SpinIrqLock::new(());
static TERMINAL_ACTIVE: AtomicBool = AtomicBool::new(false);

pub fn terminal_active() -> bool {
    TERMINAL_ACTIVE.load(Ordering::Acquire)
}

/// Called after heap initialization, before the first user process runs. The
/// allocation-free early console remains usable during boot and on early panic.
pub fn enable_terminal() {
    let _display = DISPLAY_LOCK.lock();
    if let Ok(framebuffer) = FRAME_BUFFER.try_get() {
        let mut fb = framebuffer.lock();
        if fb.terminal.is_some() {
            return;
        }
        let cols = fb.width().saturating_sub(2 * BORDER_PADDING)
            / (font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING);
        let rows = fb.height().saturating_sub(2 * BORDER_PADDING)
            / (font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING);
        fb.terminal = Some(mos_terminal::Terminal::new(cols, rows));
        fb.framebuffer.fill(0);
        fb.render_pending(true);
        TERMINAL_ACTIVE.store(true, Ordering::Release);
    }
}

pub fn terminal_size() -> (u16, u16) {
    FRAME_BUFFER
        .try_get()
        .ok()
        .and_then(|fb| {
            fb.lock().terminal.as_ref().map(|t| {
                let (cols, rows) = t.size();
                (cols as u16, rows as u16)
            })
        })
        .unwrap_or((80, 25))
}

pub fn write_terminal_and_present(bytes: &[u8], onlcr: bool, present: impl FnOnce()) {
    let _display = DISPLAY_LOCK.lock();
    if let Ok(framebuffer) = FRAME_BUFFER.try_get() {
        framebuffer.lock().write_terminal(bytes, onlcr, false);
    }
    // Drop the model lock before the compositor samples its background, while
    // retaining exclusive display ownership through both layers.
    present();
}

pub fn take_terminal_damage() -> Option<[usize; 4]> {
    FRAME_BUFFER
        .try_get()
        .ok()
        .and_then(|fb| fb.lock().damage.take())
}

/// Reconstruct covered pixels from character state, never from the framebuffer
/// (which may currently contain a window). Caller holds DISPLAY_LOCK.
pub fn paint_terminal_region(
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    mut pixel: impl FnMut(usize, usize, [u8; 3]),
) {
    let Ok(framebuffer) = FRAME_BUFFER.try_get() else {
        return;
    };
    let fb = framebuffer.lock();
    let Some(terminal) = fb.terminal.as_ref() else {
        return;
    };
    let cw = font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
    let ch = font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
    let (cols, rows) = terminal.size();
    let right = x.saturating_add(w).min(fb.width());
    let bottom = y.saturating_add(h).min(fb.height());
    let cursor = terminal.cursor();
    for row in y.saturating_sub(BORDER_PADDING) / ch
        ..bottom.saturating_sub(BORDER_PADDING).div_ceil(ch).min(rows)
    {
        for col in x.saturating_sub(BORDER_PADDING) / cw
            ..right.saturating_sub(BORDER_PADDING).div_ceil(cw).min(cols)
        {
            let cell = terminal.cell(col, row).unwrap();
            let glyph = get_char_raster_safe(cell.character);
            let x0 = BORDER_PADDING + col * cw;
            let y0 = BORDER_PADDING + row * ch;
            for py in y.max(y0)..bottom.min(y0 + ch) {
                for px in x.max(x0)..right.min(x0 + cw) {
                    let intensity = if cursor == (col, row, true) && py == y0 + ch - 2 {
                        255
                    } else {
                        glyph
                            .raster()
                            .get(py - y0)
                            .and_then(|r| r.get(px - x0))
                            .copied()
                            .unwrap_or(0)
                    };
                    let blend = |i: usize| {
                        ((cell.foreground[i] as u32 * intensity as u32
                            + cell.background[i] as u32 * (255 - intensity as u32))
                            / 255) as u8
                    };
                    pixel(px, py, [blend(0), blend(1), blend(2)]);
                }
            }
        }
    }
}

impl FrameBufferWriter {
    fn write_terminal(&mut self, bytes: &[u8], onlcr: bool, direct: bool) {
        if let Some(terminal) = self.terminal.as_mut() {
            for &byte in bytes {
                if byte == b'\n' && onlcr {
                    terminal.write(b"\r");
                }
                terminal.write(core::slice::from_ref(&byte));
            }
            self.render_pending(direct);
        }
    }

    fn render_pending(&mut self, direct: bool) {
        let Some(terminal) = self.terminal.as_ref() else {
            return;
        };
        let (cols, rows) = terminal.size();
        let cursor = terminal.cursor();
        let cw = font_constants::CHAR_RASTER_WIDTH + LETTER_SPACING;
        let ch = font_constants::CHAR_RASTER_HEIGHT.val() + LINE_SPACING;
        for index in 0..cols * rows {
            let Some(cell) = self.terminal.as_mut().unwrap().take_dirty(index) else {
                continue;
            };
            let col = index % cols;
            let row = index / cols;
            let x0 = BORDER_PADDING + col * cw;
            let y0 = BORDER_PADDING + row * ch;
            self.damage = Some(match self.damage {
                Some([x, y, w, h]) => {
                    let left = x.min(x0);
                    let top = y.min(y0);
                    [
                        left,
                        top,
                        (x + w).max(x0 + cw) - left,
                        (y + h).max(y0 + ch) - top,
                    ]
                }
                None => [x0, y0, cw, ch],
            });
            // Normal TTY output is painted once by the compositor. Only boot
            // and emergency text draws directly into the framebuffer.
            if !direct {
                continue;
            }
            self.cell_colors = Some((cell.foreground, cell.background));
            let glyph = get_char_raster_safe(cell.character);
            for y in 0..ch {
                for x in 0..cw {
                    let intensity = glyph
                        .raster()
                        .get(y)
                        .and_then(|r| r.get(x))
                        .copied()
                        .unwrap_or(0);
                    self.write_pixel(x0 + x, y0 + y, intensity);
                }
            }
            if cursor == (col, row, true) {
                for x in 0..cw {
                    self.write_pixel(x0 + x, y0 + ch - 2, 255);
                }
            }
        }
        self.cell_colors = None;
    }
}
pub fn init(fb: &'static mut FrameBuffer) -> FrameBufferInfo {
    let info = fb.info();
    let bf = fb.buffer_mut();
    FRAME_BUFFER.init_once(|| SpinIrqLock::new(FrameBufferWriter::new(bf, info)));
    info
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    let _display = DISPLAY_LOCK.lock();
    if let Ok(f) = FRAME_BUFFER.try_get() {
        let mut guard = f.lock();
        let _ = guard.write_fmt(args);
    }
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
