#[allow(dead_code)]
use core::fmt::Write;
use vga::colors::TextModeColor;
use vga::fonts::TEXT_8X16_FONT;
use vga::vga::{VGA, VideoMode};
use vga::writers::{Screen, ScreenCharacter, TextWriter};

pub type Color = vga::colors::Color16;
pub type ColorCode = TextModeColor;
type ScreenChar = ScreenCharacter;

const BUFFER_HEIGHT: usize = vga::writers::Text80x25::HEIGHT;
const BUFFER_WIDTH: usize = vga::writers::Text80x25::WIDTH;

static TEXT80X25: vga::writers::Text80x25 = vga::writers::Text80x25;

pub struct Console {
    column_position: usize,
    color_code: ColorCode,
}

impl Console {
    fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                TEXT80X25.write_character(
                    self.column_position,
                    BUFFER_HEIGHT - 1,
                    ScreenCharacter::new(byte, self.color_code),
                );

                self.column_position += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let character = TEXT80X25.read_character(col, row);
                TEXT80X25.write_character(col, row - 1, character);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.column_position = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar::new(b' ', self.color_code);
        for col in 0..BUFFER_WIDTH {
            TEXT80X25.write_character(col, row, blank);
        }
    }

    fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                // 可以是能打印的 ASCII 码字节，也可以是换行符
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                // 不包含在上述范围之内的字节
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn with_color(&mut self, code_color: ColorCode, f: impl FnOnce(&mut Console)) -> &mut Self {
        let temp = self.color_code;
        self.color_code = code_color;
        f(self);
        self.color_code = temp;
        self
    }
}

pub fn init() {
    // let mut vga = VGA.lock();
    // vga.set_video_mode(VideoMode::Mode80x25);
    {
        let mut vga = VGA.lock();
        vga.set_video_mode(VideoMode::Mode80x25);
        vga.load_font(&TEXT_8X16_FONT);
    }
    TEXT80X25.fill_screen(ScreenCharacter::new(
        b' ',
        TextModeColor::new(vga::colors::Color16::Black, vga::colors::Color16::Black),
    ))
}

impl Write for Console {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // if let Some(VideoMode::Mode80x25) | None = vga::vga::VGA.lock().get_most_recent_video_mode() {
        self.write_string(s);
        // }
        Ok(())
    }
}

/// Be careful, because it's shared globally, it's locked
/// It will deadlock when use writer and println at the same time
pub static CONSOLE: spin::Mutex<Console> = spin::Mutex::new(Console {
    column_position: 0,
    color_code: ColorCode::new(Color::LightGreen, Color::Black),
});

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::driver::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    CONSOLE
        .lock()
        .write_fmt(args)
        .expect("Printing to VGA failed");
}
