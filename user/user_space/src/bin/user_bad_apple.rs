#![no_std]
#![no_main]

use user_lib::mos::fs::O_RDONLY;
use user_lib::{clock_us, create_window, exit, open, read, sleep, window_mark_dirty, window_sync};

type RGBA = [u8; 4];

const WHITE: RGBA = [255, 255, 255, 255];
const BLACK: RGBA = [0, 0, 0, 255];

const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 200;
const FRAME_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT / 8;
const LINE_BYTES: usize = FRAME_SIZE / VIDEO_HEIGHT;
const FRAME_INTERVAL_MS: u64 = 1000 / 15; // ~66ms @ 15fps

#[unsafe(no_mangle)]
fn main() -> i32 {
    let info = match create_window(1000, 200, VIDEO_WIDTH as u64, VIDEO_HEIGHT as u64) {
        Some(i) => i,
        None => return -1,
    };
    let pitch = info.pitch as usize;
    let buf = unsafe {
        core::slice::from_raw_parts_mut(info.pixels_ptr as *mut RGBA, pitch * VIDEO_HEIGHT)
    };

    let fd = match open(c"/dev/embedding", O_RDONLY) {
        Some(fd) => fd,
        None => return -1,
    };

    let mut frame_buf = [0u8; FRAME_SIZE];

    sleep(1000);

    let start_ms = clock_us() / 1000;
    let mut frame_index: u64 = 0;

    while read(fd, &mut frame_buf).unwrap_or(0) == FRAME_SIZE {
        for (line_no, line_bs) in frame_buf.chunks_exact(LINE_BYTES).enumerate() {
            let row_off = line_no * pitch;
            for x in 0..VIDEO_WIDTH {
                let c = x >> 3;
                let o = x & 7;
                buf[row_off + x] = if line_bs[c] & ((1 << 7) >> o) != 0 {
                    WHITE
                } else {
                    BLACK
                };
            }
        }

        window_mark_dirty(info.id, 0, 0, VIDEO_WIDTH as u64, VIDEO_HEIGHT as u64);
        window_sync();

        let elapsed_ms = clock_us() / 1000 - start_ms;
        let target_ms = frame_index * FRAME_INTERVAL_MS;
        if target_ms > elapsed_ms {
            sleep(target_ms - elapsed_ms);
        }

        frame_index += 1;
    }

    exit(0)
}
