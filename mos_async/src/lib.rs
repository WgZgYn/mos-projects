#![no_std]

extern crate alloc;

use log::info;

mod executor;
pub mod keyboard;
mod sleep;
mod task;

pub async fn test_sleep() {
    for _ in 0..5 {
        info!("test sleep 5s ...");
        sleep::sleep(1000 * 5).await;
        info!("async sleep 5s finished ...");
    }
}
