#![no_std]

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
#[repr(u16)]
pub enum Magic {
    P1 = u16::from_be_bytes(*b"P1"),
    P2,
    P3,
    P4,
    P5,
    P6,
}

impl Magic {
    #[inline]
    fn from_bytes(m: &[u8]) -> Result<Self, ParseError> {
        if m.len() < 2 || m[0] != b'P' {
            return Err(ParseError::MagicNumberError);
        }
        match m[1] {
            b'1' => Ok(Magic::P1),
            b'2' => Ok(Magic::P2),
            b'3' => Ok(Magic::P3),
            b'4' => Ok(Magic::P4),
            b'5' => Ok(Magic::P5),
            b'6' => Ok(Magic::P6),
            _ => Err(ParseError::MagicNumberError),
        }
    }
}

pub struct PPM<'a> {
    magic: Magic,
    width: u32,
    height: u32,
    max_colors: u32,
    data: &'a [u8],
}

pub enum ParseError {
    NotEnoughBytes,
    MagicNumberError,
    ILLegalChar,
}

fn read_magic(data: &[u8]) -> Result<Magic, ParseError> {
    let magic = &data.get(..2).ok_or(ParseError::NotEnoughBytes)?;
    Magic::from_bytes(magic)
}

fn read_width_height<'a>(
    sections: &mut impl Iterator<Item = &'a [u8]>,
) -> Result<(u32, u32), ParseError> {
    let line = sections.next().ok_or(ParseError::NotEnoughBytes)?;
    let s = core::str::from_utf8(line).map_err(|_| ParseError::ILLegalChar)?;
    let mut s = s
        .split_whitespace()
        .filter(|s| !s.is_empty())
        .map(|s| s.trim())
        .map(|s| s.parse::<u32>().map_err(|_| ParseError::ILLegalChar));
    let width = s.next().ok_or(ParseError::NotEnoughBytes)??;
    let height = s.next().ok_or(ParseError::NotEnoughBytes)??;
    Ok((width, height))
}

fn read_colors<'a>(sections: &mut impl Iterator<Item = &'a [u8]>) -> Result<u32, ParseError> {
    let line = sections.next().ok_or(ParseError::NotEnoughBytes)?;
    let s = core::str::from_utf8(line.trim_ascii()).map_err(|_| ParseError::ILLegalChar)?;
    let colors = s.parse::<u32>().map_err(|_| ParseError::ILLegalChar)?;
    Ok(colors)
}

impl<'a> PPM<'a> {
    pub fn from_bytes(data: &'a [u8]) -> Result<PPM<'a>, ParseError> {
        let magic = read_magic(&data)?;
        let mut section = data
            .split(|&b| b == b'\n')
            .filter(|s| !s.is_empty() && !s.starts_with(&[b'#']))
            .skip(1);
        let (width, height) = read_width_height(&mut section)?;
        let max_colors = read_colors(&mut section)?;
        let data = section.next().ok_or(ParseError::NotEnoughBytes)?;
        Ok(PPM {
            magic,
            width,
            height,
            max_colors,
            data,
        })
    }

    pub fn pixels(&self, f: impl FnMut((u32, u32), u8)) {}
}
