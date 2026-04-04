use core::pin::Pin;
use core::task::{Context, Poll};

pub struct Sleep {
    awake_tick: u64,
    registered: bool,
}

impl Sleep {
    pub fn new(sleep_ms: u64) -> Sleep {
        Sleep {
            awake_tick: get_tick() + sleep_ms,
            registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        let tick = get_tick();
        if tick >= self.awake_tick {
            Poll::Ready(())
        } else {
            if !self.registered {
                add_timer(self.awake_tick, Some(ctx.waker().clone()));
                self.registered = true;
            }

            Poll::Pending
        }
    }
}

pub async fn sleep(ms: u64) {
    Sleep::new(ms).await
}
