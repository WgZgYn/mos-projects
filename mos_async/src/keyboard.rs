use core::{
    pin::Pin,
    task::{Context, Poll},
};
use futures_util::stream::Stream;
use futures_util::task::AtomicWaker;
use mos_driver::keyboard::{try_get_key_code, try_init_once_scancode_queue};

static WAKER: AtomicWaker = AtomicWaker::new();

pub struct ScancodeStream {
    _private: (),
}
impl ScancodeStream {
    fn new() -> Self {
        try_init_once_scancode_queue();
        Self { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Option<u8>> {
        // fast path
        if let Some(scancode) = try_get_key_code() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(cx.waker());
        match try_get_key_code() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
