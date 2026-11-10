#![no_std]
//! Hardware-independent, bounded terminal screen. No syscall, font or device
//! dependencies: the same byte-stream model can later live in a userspace PTY client.
extern crate alloc;
use alloc::{vec, vec::Vec};

pub type Color = [u8; 3];
pub const PALETTE: [Color; 16] = [
    [0, 0, 0],
    [170, 0, 0],
    [0, 170, 0],
    [170, 85, 0],
    [0, 0, 170],
    [170, 0, 170],
    [0, 170, 170],
    [170, 170, 170],
    [85, 85, 85],
    [255, 85, 85],
    [85, 255, 85],
    [255, 255, 85],
    [85, 85, 255],
    [255, 85, 255],
    [85, 255, 255],
    [255, 255, 255],
];
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cell {
    pub character: char,
    pub foreground: Color,
    pub background: Color,
}
#[derive(Clone, Copy)]
struct Style {
    fg: Color,
    bg: Color,
    bold: bool,
    inverse: bool,
}
impl Style {
    const DEFAULT: Self = Self {
        fg: PALETTE[7],
        bg: PALETTE[0],
        bold: false,
        inverse: false,
    };
    fn cell(self, character: char) -> Cell {
        let mut fg = self.fg;
        if self.bold {
            if let Some(index) = PALETTE[..8].iter().position(|c| *c == fg) {
                fg = PALETTE[index + 8];
            }
        }
        let (foreground, background) = if self.inverse {
            (self.bg, fg)
        } else {
            (fg, self.bg)
        };
        Cell {
            character,
            foreground,
            background,
        }
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Ground,
    Escape,
    Csi,
    String,
    StringEscape,
    Designate,
}

pub struct Terminal {
    cols: usize,
    rows: usize,
    cells: Vec<Cell>,
    dirty: Vec<bool>,
    x: usize,
    y: usize,
    saved: (usize, usize, Style),
    style: Style,
    wrap: bool,
    cursor_visible: bool,
    state: State,
    params: [u16; 16],
    param: usize,
    private: bool,
    ignore: bool,
    utf8: [u8; 4],
    utf8_len: usize,
    utf8_need: usize,
}
impl Terminal {
    /// Limit dimensions and storage, independently of untrusted escape sequences.
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.clamp(1, 512);
        let rows = rows.clamp(1, 256);
        Self {
            cols,
            rows,
            cells: vec![Style::DEFAULT.cell(' '); cols * rows],
            dirty: vec![true; cols * rows],
            x: 0,
            y: 0,
            saved: (0, 0, Style::DEFAULT),
            style: Style::DEFAULT,
            wrap: false,
            cursor_visible: true,
            state: State::Ground,
            params: [0; 16],
            param: 0,
            private: false,
            ignore: false,
            utf8: [0; 4],
            utf8_len: 0,
            utf8_need: 0,
        }
    }
    pub fn size(&self) -> (usize, usize) {
        (self.cols, self.rows)
    }
    pub fn cursor(&self) -> (usize, usize, bool) {
        (self.x, self.y, self.cursor_visible)
    }
    pub fn cell(&self, x: usize, y: usize) -> Option<Cell> {
        if x >= self.cols || y >= self.rows {
            None
        } else {
            Some(self.cells[y * self.cols + x])
        }
    }
    pub fn take_dirty(&mut self, index: usize) -> Option<Cell> {
        if index < self.cells.len() && core::mem::replace(&mut self.dirty[index], false) {
            Some(self.cells[index])
        } else {
            None
        }
    }
    pub fn invalidate(&mut self) {
        self.dirty.fill(true);
    }
    pub fn reset(&mut self) {
        self.style = Style::DEFAULT;
        self.saved = (0, 0, Style::DEFAULT);
        self.x = 0;
        self.y = 0;
        self.wrap = false;
        self.cursor_visible = true;
        self.state = State::Ground;
        self.utf8_len = 0;
        self.utf8_need = 0;
        self.erase(0, self.cells.len());
    }
    pub fn write(&mut self, bytes: &[u8]) {
        self.dirty[self.y * self.cols + self.x] = true;
        for &byte in bytes {
            self.byte(byte);
        }
        self.dirty[self.y * self.cols + self.x] = true;
    }
    fn erase(&mut self, start: usize, end: usize) {
        self.cells[start..end].fill(self.style.cell(' '));
        self.dirty[start..end].fill(true);
    }
    fn move_to(&mut self, x: usize, y: usize) {
        self.x = x.min(self.cols - 1);
        self.y = y.min(self.rows - 1);
        self.wrap = false;
    }
    fn scroll_up(&mut self, count: usize) {
        let n = count.min(self.rows) * self.cols;
        self.cells.copy_within(n.., 0);
        self.erase(self.cells.len() - n, self.cells.len());
        self.invalidate();
    }
    fn scroll_down(&mut self, count: usize) {
        let n = count.min(self.rows) * self.cols;
        let end = self.cells.len() - n;
        self.cells.copy_within(..end, n);
        self.erase(0, n);
        self.invalidate();
    }
    fn linefeed(&mut self) {
        self.wrap = false;
        if self.y + 1 == self.rows {
            self.scroll_up(1);
        } else {
            self.y += 1;
        }
    }
    fn put(&mut self, c: char) {
        if self.wrap {
            self.x = 0;
            self.linefeed();
        }
        let i = self.y * self.cols + self.x;
        self.cells[i] = self.style.cell(c);
        self.dirty[i] = true;
        if self.x + 1 == self.cols {
            self.wrap = true;
        } else {
            self.x += 1;
        }
    }
    fn byte(&mut self, b: u8) {
        // UTF-8 survives write boundaries. A malformed continuation replaces the
        // incomplete scalar and then reprocesses this byte (including ESC/CR).
        if self.state == State::Ground && self.utf8_len != 0 {
            if b & 0xc0 == 0x80 {
                self.utf8[self.utf8_len] = b;
                self.utf8_len += 1;
                if self.utf8_len == self.utf8_need {
                    let c = core::str::from_utf8(&self.utf8[..self.utf8_len])
                        .ok()
                        .and_then(|s| s.chars().next())
                        .unwrap_or('\u{fffd}');
                    self.utf8_len = 0;
                    self.put(c);
                }
                return;
            }
            self.utf8_len = 0;
            self.put('\u{fffd}');
        }
        if b == 0x18 || b == 0x1a {
            self.state = State::Ground;
            return;
        }
        if matches!(self.state, State::String | State::StringEscape) {
            self.state = match (self.state, b) {
                (_, 7) | (State::StringEscape, b'\\') => State::Ground,
                (_, 27) => State::StringEscape,
                _ => State::String,
            };
            return;
        }
        if b == 27 {
            self.state = State::Escape;
            return;
        }
        if b < 32 || b == 127 {
            match b {
                b'\r' => self.move_to(0, self.y),
                b'\n' | 11 | 12 => self.linefeed(),
                8 => self.move_to(self.x.saturating_sub(1), self.y),
                9 => self.move_to((self.x / 8 + 1) * 8, self.y),
                _ => {}
            };
            return;
        }
        match self.state {
            State::Ground => {
                if b < 128 {
                    self.put(b as char);
                } else {
                    self.utf8_need = match b {
                        0xc2..=0xdf => 2,
                        0xe0..=0xef => 3,
                        0xf0..=0xf4 => 4,
                        _ => 0,
                    };
                    if self.utf8_need == 0 {
                        self.put('\u{fffd}');
                    } else {
                        self.utf8[0] = b;
                        self.utf8_len = 1;
                    }
                }
            }
            State::Escape => {
                self.state = State::Ground;
                match b {
                    b'[' => {
                        self.params.fill(0);
                        self.param = 0;
                        self.private = false;
                        self.ignore = false;
                        self.state = State::Csi;
                    }
                    b']' | b'P' | b'^' | b'_' => self.state = State::String,
                    b'(' | b')' | b'%' | b'#' => self.state = State::Designate,
                    b'7' => self.saved = (self.x, self.y, self.style),
                    b'8' => {
                        self.move_to(self.saved.0, self.saved.1);
                        self.style = self.saved.2;
                    }
                    b'D' => self.linefeed(),
                    b'E' => {
                        self.move_to(0, self.y);
                        self.linefeed();
                    }
                    b'M' => {
                        if self.y == 0 {
                            self.scroll_down(1);
                        } else {
                            self.y -= 1;
                        }
                        self.wrap = false;
                    }
                    b'c' => self.reset(),
                    _ => {}
                }
            }
            State::Designate => self.state = State::Ground,
            State::Csi => match b {
                b'0'..=b'9' => {
                    let value = self.params[self.param]
                        .checked_mul(10)
                        .and_then(|v| v.checked_add((b - b'0') as u16));
                    if let Some(v) = value {
                        self.params[self.param] = v;
                    } else {
                        self.ignore = true;
                    }
                }
                b';' => {
                    if self.param < 15 {
                        self.param += 1;
                    } else {
                        self.ignore = true;
                    }
                }
                b'?' if self.param == 0 && self.params[0] == 0 && !self.private => {
                    self.private = true
                }
                0x40..=0x7e => {
                    if !self.ignore {
                        self.csi(b);
                    }
                    self.state = State::Ground;
                }
                _ => self.ignore = true,
            },
            State::String | State::StringEscape => unreachable!(),
        }
    }
    fn count(&self, index: usize) -> usize {
        self.params[index].max(1) as usize
    }
    fn csi(&mut self, final_byte: u8) {
        if self.private {
            if self.params[0] == 25 && self.param == 0 {
                match final_byte {
                    b'h' => self.cursor_visible = true,
                    b'l' => self.cursor_visible = false,
                    _ => {}
                }
            }
            return;
        }
        let n = self.count(0);
        let i = self.y * self.cols + self.x;
        match final_byte {
            b'A' => self.move_to(self.x, self.y.saturating_sub(n)),
            b'B' => self.move_to(self.x, self.y + n),
            b'C' => self.move_to(self.x + n, self.y),
            b'D' => self.move_to(self.x.saturating_sub(n), self.y),
            b'E' => self.move_to(0, self.y + n),
            b'F' => self.move_to(0, self.y.saturating_sub(n)),
            b'G' | b'`' => self.move_to(n - 1, self.y),
            b'd' => self.move_to(self.x, n - 1),
            b'H' | b'f' => self.move_to(self.count(1) - 1, n - 1),
            b'J' => match self.params[0] {
                0 => self.erase(i, self.cells.len()),
                1 => self.erase(0, i + 1),
                2 | 3 => self.erase(0, self.cells.len()),
                _ => {}
            },
            b'K' => match self.params[0] {
                0 => self.erase(i, (self.y + 1) * self.cols),
                1 => self.erase(self.y * self.cols, i + 1),
                2 => self.erase(self.y * self.cols, (self.y + 1) * self.cols),
                _ => {}
            },
            b'X' => self.erase(i, (i + n).min((self.y + 1) * self.cols)),
            b'P' => {
                let end = (self.y + 1) * self.cols;
                let n = n.min(end - i);
                self.cells.copy_within(i + n..end, i);
                self.erase(end - n, end);
                self.dirty[i..end].fill(true);
            }
            b'@' => {
                let end = (self.y + 1) * self.cols;
                let n = n.min(end - i);
                self.cells.copy_within(i..end - n, i + n);
                self.erase(i, i + n);
                self.dirty[i..end].fill(true);
            }
            b'S' => self.scroll_up(n),
            b'T' => self.scroll_down(n),
            b's' => self.saved = (self.x, self.y, self.style),
            b'u' => {
                self.move_to(self.saved.0, self.saved.1);
                self.style = self.saved.2;
            }
            b'm' => self.sgr(),
            _ => {}
        }
    }
    fn sgr(&mut self) {
        let mut i = 0;
        while i <= self.param {
            match self.params[i] {
                0 => self.style = Style::DEFAULT,
                1 => self.style.bold = true,
                22 => self.style.bold = false,
                7 => self.style.inverse = true,
                27 => self.style.inverse = false,
                30..=37 => self.style.fg = PALETTE[(self.params[i] - 30) as usize],
                40..=47 => self.style.bg = PALETTE[(self.params[i] - 40) as usize],
                90..=97 => self.style.fg = PALETTE[(self.params[i] - 90 + 8) as usize],
                100..=107 => self.style.bg = PALETTE[(self.params[i] - 100 + 8) as usize],
                39 => self.style.fg = Style::DEFAULT.fg,
                49 => self.style.bg = Style::DEFAULT.bg,
                38 | 48 => {
                    let foreground = self.params[i] == 38;
                    let color = if i + 2 <= self.param && self.params[i + 1] == 5 {
                        i += 2;
                        Some(indexed_color(self.params[i].min(255) as u8))
                    } else if i + 4 <= self.param && self.params[i + 1] == 2 {
                        let c = [
                            self.params[i + 2].min(255) as u8,
                            self.params[i + 3].min(255) as u8,
                            self.params[i + 4].min(255) as u8,
                        ];
                        i += 4;
                        Some(c)
                    } else {
                        break;
                    };
                    if let Some(c) = color {
                        if foreground {
                            self.style.fg = c;
                        } else {
                            self.style.bg = c;
                        }
                    }
                }
                _ => {}
            }
            i += 1;
        }
    }
}
fn indexed_color(index: u8) -> Color {
    if index < 16 {
        PALETTE[index as usize]
    } else if index >= 232 {
        [8 + (index - 232) * 10; 3]
    } else {
        let n = index - 16;
        let v = |x| if x == 0 { 0 } else { 55 + x * 40 };
        [v(n / 36), v(n / 6 % 6), v(n % 6)]
    }
}

#[cfg(test)]
mod tests;
