#![no_std]

use bitflags::bitflags;

pub const TER_V16N: &[u8] = include_bytes!("../assert/font/ter-v16n.psf");
/// PSF 版本
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PsfVersion {
    V1,
    V2,
}

/// PSF 头标志位
bitflags! {
    pub struct PsfFlags: u32 {
        const UNICODE_TABLE = 0x01;
    }
}

/// PSF1 文件头 (32 bytes)
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct Psf1Header {
    pub magic: u16,    // 0x0436
    pub mode: u8,      // 字体模式
    pub char_size: u8, // 字符大小（字节）
}

/// PSF2 文件头 (32 bytes)
#[repr(C, packed)]
#[derive(Debug, Copy, Clone)]
pub struct Psf2Header {
    pub magic: u32,       // 0x864ab572
    pub version: u32,     // 版本号
    pub header_size: u32, // 头大小
    pub flags: u32,       // 标志位
    pub length: u32,      // 字符数量
    pub char_size: u32,   // 每个字符的字节数
    pub height: u32,      // 字符高度（像素）
    pub width: u32,       // 字符宽度（像素）
}

/// PSF 字体解析错误
#[derive(Debug, Clone, PartialEq)]
pub enum PsfError {
    InvalidMagic,
    UnsupportedVersion,
    InvalidHeader,
    OutOfBounds,
    NoUnicodeTable,
}

/// PSF 字体解析器
pub struct PsfFont<'a> {
    data: &'a [u8],
    version: PsfVersion,
    char_count: usize,
    char_size: usize,
    pub width: usize,
    pub height: usize,
    flags: PsfFlags,
    unicode_table: Option<&'a [u8]>,
}

impl<'a> PsfFont<'a> {
    /// 从字节切片创建 PSF 字体解析器
    pub fn new(data: &'a [u8]) -> Result<Self, PsfError> {
        if data.len() < 4 {
            return Err(PsfError::InvalidHeader);
        }

        // 检查魔数以确定版本
        let magic_u16 = u16::from_le_bytes([data[0], data[1]]);
        let magic_u32 = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        match (magic_u16, magic_u32) {
            (0x0436, _) => Self::parse_psf1(data),
            (_, 0x864ab572) => Self::parse_psf2(data),
            _ => Err(PsfError::InvalidMagic),
        }
    }

    /// 解析 PSF1 字体
    fn parse_psf1(data: &'a [u8]) -> Result<Self, PsfError> {
        if data.len() < core::mem::size_of::<Psf1Header>() {
            return Err(PsfError::InvalidHeader);
        }

        // 安全地读取头（使用指针转换避免对齐问题）
        let header = unsafe { &*(data.as_ptr() as *const Psf1Header) };

        if header.magic != 0x0436 {
            return Err(PsfError::InvalidMagic);
        }

        let char_size = header.char_size as usize;
        let char_count = (data.len() - core::mem::size_of::<Psf1Header>()) / char_size;

        // PSF1 通常是 8x16 或 8x14
        let (width, height) = match char_size {
            16 => (8, 16),       // 8x16
            14 => (8, 14),       // 8x14
            _ => (8, char_size), // 假设宽度为8
        };

        Ok(Self {
            data,
            version: PsfVersion::V1,
            char_count,
            char_size,
            width,
            height,
            flags: PsfFlags::empty(),
            unicode_table: None, // PSF1 通常没有 Unicode 表
        })
    }

    /// 解析 PSF2 字体
    fn parse_psf2(data: &'a [u8]) -> Result<Self, PsfError> {
        if data.len() < size_of::<Psf2Header>() {
            return Err(PsfError::InvalidHeader);
        }

        let header = unsafe { &*(data.as_ptr() as *const Psf2Header) };

        if header.magic != 0x864ab572 {
            return Err(PsfError::InvalidMagic);
        }

        let header_size = header.header_size as usize;
        let char_count = header.length as usize;
        let char_size = header.char_size as usize;
        let width = header.width as usize;
        let height = header.height as usize;
        let flags = PsfFlags::from_bits(header.flags).unwrap_or(PsfFlags::empty());

        // 计算字体数据开始位置
        let font_data_start = header_size;
        let font_data_end = font_data_start + char_count * char_size;

        if font_data_end > data.len() {
            return Err(PsfError::OutOfBounds);
        }

        // 处理 Unicode 表
        let unicode_table = if flags.contains(PsfFlags::UNICODE_TABLE) {
            let table_start = font_data_end;
            if table_start < data.len() {
                Some(&data[table_start..])
            } else {
                None
            }
        } else {
            None
        };

        Ok(Self {
            data,
            version: PsfVersion::V2,
            char_count,
            char_size,
            width,
            height,
            flags,
            unicode_table,
        })
    }

    /// 获取字符数量
    pub fn char_count(&self) -> usize {
        self.char_count
    }

    /// 获取字符尺寸（宽 x 高）
    pub fn char_size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// 获取版本
    pub fn version(&self) -> PsfVersion {
        self.version
    }

    /// 检查是否支持 Unicode
    pub fn has_unicode(&self) -> bool {
        self.unicode_table.is_some()
    }

    /// 获取字符的位图数据
    pub fn get_char(&self, index: usize) -> Result<&[u8], PsfError> {
        if index >= self.char_count {
            return Err(PsfError::OutOfBounds);
        }

        let start = match self.version {
            PsfVersion::V1 => core::mem::size_of::<Psf1Header>(),
            PsfVersion::V2 => {
                let header = unsafe { &*(self.data.as_ptr() as *const Psf2Header) };
                header.header_size as usize
            }
        } + index * self.char_size;

        let end = start + self.char_size;

        if end > self.data.len() {
            return Err(PsfError::OutOfBounds);
        }

        Ok(&self.data[start..end])
    }

    /// 通过 Unicode 码点查找字符
    pub fn find_char_by_unicode(&self, code_point: u32) -> Option<usize> {
        let table = self.unicode_table?;

        let mut current_index = 0;
        let mut table_pos = 0;

        while table_pos < table.len() && current_index < self.char_count {
            // 解析 UTF-8 序列
            let (seq_len, decoded) = self.decode_utf8(&table[table_pos..])?;
            table_pos += seq_len;

            if decoded == 0xFFFF {
                // 分隔符，移动到下一个字符
                current_index += 1;
            } else if decoded == code_point {
                return Some(current_index);
            }
        }

        None
    }

    /// 解码 UTF-8 序列
    fn decode_utf8(&self, data: &[u8]) -> Option<(usize, u32)> {
        if data.is_empty() {
            return None;
        }

        let first = data[0];
        if first < 0x80 {
            // 1字节 UTF-8
            Some((1, first as u32))
        } else if (first & 0xE0) == 0xC0 && data.len() >= 2 {
            // 2字节 UTF-8
            let code_point = ((first as u32 & 0x1F) << 6) | (data[1] as u32 & 0x3F);
            Some((2, code_point))
        } else if (first & 0xF0) == 0xE0 && data.len() >= 3 {
            // 3字节 UTF-8
            let code_point = ((first as u32 & 0x0F) << 12)
                | ((data[1] as u32 & 0x3F) << 6)
                | (data[2] as u32 & 0x3F);
            Some((3, code_point))
        } else if (first & 0xF8) == 0xF0 && data.len() >= 4 {
            // 4字节 UTF-8
            let code_point = ((first as u32 & 0x07) << 18)
                | ((data[1] as u32 & 0x3F) << 12)
                | ((data[2] as u32 & 0x3F) << 6)
                | (data[3] as u32 & 0x3F);
            Some((4, code_point))
        } else {
            None
        }
    }

    /// 渲染字符到位图缓冲区
    pub fn render_char(
        &self,
        index: usize,
        buffer: &mut [u8],
        buffer_width: usize,
    ) -> Result<(), PsfError> {
        let char_data = self.get_char(index)?;

        for y in 0..self.height {
            for x in 0..self.width {
                let byte_index = y * ((self.width + 7) / 8);
                let bit_index = 7 - (x % 8);
                let mask = 1 << bit_index;

                let pixel_set = (char_data[byte_index] & mask) != 0;
                let buffer_index = y * buffer_width + x;

                if buffer_index < buffer.len() {
                    buffer[buffer_index] = if pixel_set { 0xFF } else { 0x00 };
                }
            }
        }

        Ok(())
    }
}

/// 字体渲染器
pub struct FontRenderer<'a> {
    font: &'a PsfFont<'a>,
    foreground: u32,
    background: u32,
}

impl<'a> FontRenderer<'a> {
    pub fn new(font: &'a PsfFont) -> Self {
        Self {
            font,
            foreground: 0xFFFFFF, // 白色
            background: 0x000000, // 黑色
        }
    }

    /// 设置前景色（RGB）
    pub fn set_foreground(&mut self, color: u32) {
        self.foreground = color;
    }

    /// 设置背景色（RGB）
    pub fn set_background(&mut self, color: u32) {
        self.background = color;
    }

    /// 渲染字符到 RGBA 缓冲区
    pub fn render_char_rgba(
        &self,
        index: usize,
        buffer: &mut [u32],
        buffer_width: usize,
    ) -> Result<(), PsfError> {
        let (char_width, char_height) = self.font.char_size();
        let char_data = self.font.get_char(index)?;

        for y in 0..char_height {
            for x in 0..char_width {
                let byte_index = y * ((char_width + 7) / 8);
                let bit_index = 7 - (x % 8);
                let mask = 1 << bit_index;

                let pixel_set = (char_data[byte_index] & mask) != 0;
                let buffer_index = y * buffer_width + x;

                if buffer_index < buffer.len() {
                    buffer[buffer_index] = if pixel_set {
                        self.foreground
                    } else {
                        self.background
                    };
                }
            }
        }

        Ok(())
    }

    /// 渲染字符串
    pub fn render_string_rgba(
        &self,
        text: &str,
        buffer: &mut [u32],
        buffer_width: usize,
        x: usize,
        y: usize,
    ) -> Result<(), PsfError> {
        let (char_width, char_height) = self.font.char_size();
        let mut current_x = x;

        for ch in text.chars() {
            if current_x + char_width > buffer_width {
                break;
            }

            if let Some(index) = self.font.find_char_by_unicode(ch as u32) {
                let char_buffer_start = y * buffer_width + current_x;
                let char_buffer_end = char_buffer_start + char_height * buffer_width;

                if char_buffer_end <= buffer.len() {
                    let char_buffer = &mut buffer[char_buffer_start..char_buffer_end];
                    self.render_char_rgba(index, char_buffer, buffer_width)?;
                }
            }

            current_x += char_width;
        }

        Ok(())
    }
}
