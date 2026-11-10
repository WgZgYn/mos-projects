use crate::{Rect, RGBA};

pub trait PixelSink {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn blit_row(&mut self, pos_x: usize, pos_y: usize, src: &[RGBA]);
    /// Optional retained desktop/terminal layer beneath all windows.
    fn restore_background(&mut self, _rect: Rect) {}
}

pub trait PixelBuffer {
    fn width(&self) -> usize;
    fn height(&self) -> usize;
    fn row(&self, y: usize) -> Option<&[RGBA]>;
    fn row_mut(&mut self, y: usize) -> Option<&mut [RGBA]>;
}

pub trait PixelWindow {
    fn x_start(&self) -> usize;
    fn y_start(&self) -> usize;
    #[inline]
    fn bound(&self) -> Rect {
        Rect {
            x: self.x_start(),
            y: self.y_start(),
            w: self.buffer().width(),
            h: self.buffer().height(),
        }
    }
    fn buffer_mut(&mut self) -> &mut impl PixelBuffer;
    fn buffer(&self) -> &impl PixelBuffer;
    fn add_dirty(&mut self, rect: Rect);
    fn take_dirty(&mut self) -> impl Iterator<Item = Rect>;
}

pub trait Compositor {
    fn flush(&mut self, sink: &mut impl PixelSink);
}
