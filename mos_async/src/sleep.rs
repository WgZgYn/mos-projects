use core::pin::Pin;
use core::task::{Context, Poll};
use mos_platform::timer::{add_timer, deadline_expired, monotonic_deadline_after_ms};

pub struct Sleep {
    deadline: u64,
    registered: bool,
}

impl Sleep {
    pub fn new(sleep_ms: u64) -> Sleep {
        Sleep {
            deadline: monotonic_deadline_after_ms(sleep_ms),
            registered: false,
        }
    }
}

impl Future for Sleep {
    type Output = ();
    fn poll(mut self: Pin<&mut Self>, ctx: &mut Context) -> Poll<Self::Output> {
        if deadline_expired(self.deadline) {
            Poll::Ready(())
        } else {
            if !self.registered {
                self.registered = add_timer(self.deadline, Some(ctx.waker().clone()));
            }

            Poll::Pending
        }
    }
}

pub async fn sleep(ms: u64) {
    Sleep::new(ms).await
}
