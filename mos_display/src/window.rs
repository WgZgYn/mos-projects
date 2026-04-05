use crate::traits::{PixelBuffer, PixelWindow};
use crate::{RGBA, Rect};
use alloc::vec;
use alloc::vec::Vec;

pub struct Window<Buffer: PixelBuffer> {
    x: usize,
    y: usize,
    buffer: Buffer,
    dirty: Vec<Rect>,
}

pub struct Surface {
    width: usize,
    height: usize,
    buffer: Vec<Vec<RGBA>>,
}

impl PixelBuffer for Surface {
    #[inline]
    fn width(&self) -> usize {
        self.width
    }

    #[inline]
    fn height(&self) -> usize {
        self.height
    }

    #[inline]
    fn row(&self, y: usize) -> Option<&[RGBA]> {
        self.buffer.get(y).map(|v| v.as_slice())
    }

    #[inline]
    fn row_mut(&mut self, y: usize) -> Option<&mut [RGBA]> {
        self.buffer.get_mut(y).map(|v| v.as_mut_slice())
    }
}

impl<Buffer: PixelBuffer> PixelWindow for Window<Buffer> {
    #[inline]
    fn x_start(&self) -> usize {
        self.x
    }

    #[inline]
    fn y_start(&self) -> usize {
        self.y
    }

    #[inline]
    fn buffer_mut(&mut self) -> &mut impl PixelBuffer {
        &mut self.buffer
    }

    #[inline]
    fn buffer(&self) -> &impl PixelBuffer {
        &self.buffer
    }

    fn add_dirty(&mut self, mut rect: Rect) {
        rect.x += self.x;
        rect.y += self.y;
        self.dirty.push(rect);
    }

    fn take_dirty(&mut self) -> impl Iterator<Item = Rect> {
        self.dirty.drain(..)
    }
}

impl<Buffer: PixelBuffer> Window<Buffer> {
    pub const fn new(x: usize, y: usize, buffer: Buffer) -> Self {
        Self {
            x,
            y,
            buffer,
            dirty: Vec::new(),
        }
    }
}

impl Surface {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            buffer: vec![
                vec![
                    RGBA {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 0
                    };
                    width
                ];
                height
            ],
        }
    }
}
