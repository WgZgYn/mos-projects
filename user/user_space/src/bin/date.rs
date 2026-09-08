#![no_std]
#![no_main]

use user_lib::mos::time::{CLOCK_MONOTONIC, CLOCK_REALTIME};
use user_lib::{clock_gettime, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let rt = clock_gettime(CLOCK_REALTIME);
    let mt = clock_gettime(CLOCK_MONOTONIC);
    if rt.tv_sec < 0 {
        println!("clock_gettime(REALTIME) failed: {}", rt.tv_sec);
        return 1;
    }
    println!("realtime epoch secs = {}", rt.tv_sec);
    println!("monotonic secs     = {}", mt.tv_sec);
    // 2026 is ~1.78e9; anything past 2001-09-09 proves the RTC is read
    if rt.tv_sec > 1_000_000_000 {
        println!("RTC ok: wall clock is past 2001");
        0
    } else {
        println!("RTC suspicious: got {}", rt.tv_sec);
        1
    }
}
