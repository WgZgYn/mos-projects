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
        let Some(clipped) = rect.intersect(Rect {
            x: 0, y: 0, w: self.buffer.width(), h: self.buffer.height(),
        }) else { return; };
        rect = clipped;
        rect.x += self.x;
        rect.y += self.y;
        // Bound pending work even when an application repeatedly marks pixels
        // without presenting. One bounding rectangle is enough for correctness.
        if let Some(pending) = self.dirty.first_mut() {
            let right = (pending.x + pending.w).max(rect.x + rect.w);
            let bottom = (pending.y + pending.h).max(rect.y + rect.h);
            pending.x = pending.x.min(rect.x);
            pending.y = pending.y.min(rect.y);
            pending.w = right - pending.x;
            pending.h = bottom - pending.y;
        } else {
            self.dirty.push(rect);
        }
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

/// 用户态共享像素缓冲区 — 内核和用户进程共享同一块物理内存。
/// `ptr` 指向内核 VA，用户进程通过映射到用户 VA 的同一物理帧直接写入。
pub struct SharedSurface {
    width: usize,
    height: usize,
    pitch: usize,
    ptr: *mut RGBA,
    // Pin the backing independently of user mappings (munmap/exec/COW).
    _backing: alloc::sync::Arc<dyn Send + Sync>,
}

unsafe impl Send for SharedSurface {}
unsafe impl Sync for SharedSurface {}

impl SharedSurface {
    /// # Safety
    /// `ptr` 必须指向 width * height * 4 字节的已清零连续内存，
    /// 且在其生命周期内保持有效。
    pub unsafe fn new(width: usize, height: usize, pitch: usize, ptr: *mut RGBA,
        backing: alloc::sync::Arc<dyn Send + Sync>) -> Self {
        Self {
            width,
            height,
            pitch,
            ptr,
            _backing: backing,
        }
    }
}

impl PixelBuffer for SharedSurface {
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
        if y >= self.height {
            return None;
        }
        let start = y * self.pitch;
        Some(unsafe { core::slice::from_raw_parts(self.ptr.add(start), self.width) })
    }

    #[inline]
    fn row_mut(&mut self, y: usize) -> Option<&mut [RGBA]> {
        if y >= self.height {
            return None;
        }
        let start = y * self.pitch;
        Some(unsafe { core::slice::from_raw_parts_mut(self.ptr.add(start), self.width) })
    }
}
