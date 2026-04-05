use crate::RGBA;
use bootloader_api::info::{FrameBuffer, FrameBufferInfo, PixelFormat};
use mos_arch::sync::SpinIrqLock;
use crate::traits::PixelSink;

type BlitRowFn = fn(dst: &mut [u8], src: &[RGBA]);

fn blit_row_bgr4(dst: &mut [u8], src: &[RGBA]) {
    let mut i = 0;
    for &RGBA { r, g, b, a } in src {
        dst[i] = b;
        dst[i + 1] = g;
        dst[i + 2] = r;
        dst[i + 3] = a;
        i += 4;
    }
}

fn blit_row_rgb4(dst: &mut [u8], src: &[RGBA]) {
    unsafe {
        core::ptr::copy_nonoverlapping(src.as_ptr() as *const u8, dst.as_mut_ptr(), src.len() * 4)
    }
}

fn blit_row_bgr3(dst: &mut [u8], src: &[RGBA]) {
    let mut i = 0;
    for &RGBA { r, g, b, .. } in src {
        dst[i] = b;
        dst[i + 1] = g;
        dst[i + 2] = r;
        i += 3;
    }
}

fn blit_row_rgb3(dst: &mut [u8], src: &[RGBA]) {
    let mut i = 0;
    for &RGBA { r, g, b, .. } in src {
        dst[i] = r;
        dst[i + 1] = g;
        dst[i + 2] = b;
        i += 3;
    }
}

pub struct FrameBufferAdapter {
    buffer: &'static mut [u8],
    info: FrameBufferInfo,
    blit_row: BlitRowFn,
}

impl FrameBufferAdapter {
    fn new(framebuffer: &'static mut FrameBuffer) -> Self {
        let info = framebuffer.info();
        Self {
            buffer: framebuffer.buffer_mut(),
            blit_row: match (info.pixel_format, info.bytes_per_pixel) {
                (PixelFormat::Rgb, 3) => blit_row_rgb3,
                (PixelFormat::Rgb, 4) => blit_row_rgb4,
                (PixelFormat::Bgr, 3) => blit_row_bgr3,
                (PixelFormat::Bgr, 4) => blit_row_bgr4,
                _ => todo!(),
            },
            info,
        }
    }

    #[inline]
    fn row_mut(&mut self, row: usize) -> &mut [u8] {
        let row_bytes = self.info.stride * self.info.bytes_per_pixel;
        &mut self.buffer[row * row_bytes..(row + 1) * row_bytes]
    }

    #[inline]
    fn clear(&mut self) {
        self.buffer.fill(0);
    }

    fn fill(&mut self, rgba: RGBA) {
        let bpp = self.info.bytes_per_pixel;
        let bs = &rgba.into_target(self.info.pixel_format)[..bpp];
        for y in 0..self.info.height {
            let row = self.row_mut(y);
            for chunk in row.chunks_exact_mut(bpp) {
                chunk.copy_from_slice(bs);
            }
        }
    }
}

impl PixelSink for FrameBufferAdapter {
    fn width(&self) -> usize {
        self.info.width
    }

    fn height(&self) -> usize {
        self.info.height
    }

    fn blit_row(&mut self, pos_x: usize, pos_y: usize, src: &[RGBA]) {
        let f = self.blit_row;
        if pos_x >= self.width() || pos_y >= self.height() {
            return;
        }
        let pixel_len = src.len().min(self.width() - pos_x);
        let bytes_len = pixel_len * self.info.bytes_per_pixel;
        let bytes_offset = pos_x * self.info.bytes_per_pixel;
        f(
            &mut self.row_mut(pos_y)[bytes_offset..bytes_offset + bytes_len],
            &src[..pixel_len],
        );
    }
}

static ADAPTER: SpinIrqLock<Option<FrameBufferAdapter>> = SpinIrqLock::new(None);

pub fn init_framebuffer_adapter(
    framebuffer: &'static mut FrameBuffer,
) -> Option<FrameBufferAdapter> {
    ADAPTER.access(|a| a.replace(FrameBufferAdapter::new(framebuffer)))
}

pub fn access_framebuffer_adapter<R>(f: impl FnOnce(&mut FrameBufferAdapter) -> R) -> Option<R> {
    ADAPTER.access(|a| {
        if let Some(adapter) = a {
            Some(f(adapter))
        } else {
            None
        }
    })
}
