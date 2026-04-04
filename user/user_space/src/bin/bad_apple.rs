#![no_std]
#![no_main]
extern crate alloc;

use user_lib::println;

struct FrameBufferInfo {
    width: usize,
    height: usize,
    buffer: &'static mut [u8],
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[USER] test play bad apple");

    0
}

const VIDEO_WIDTH: usize = 320;
const VIDEO_HEIGHT: usize = 200;
const VIDEO_FPS: usize = 17;
const FRAME_SIZE: usize = VIDEO_WIDTH * VIDEO_HEIGHT / 8;
const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

pub fn play_bad_apple() {
    // buffer.fill(0);
    // for (i, frame) in RAW_BPP.as_chunks::<FRAME_SIZE>().0.iter().enumerate() {
    //     let start = get_tick();
    //     write_frame(info, buffer, frame);
    //     let target = start + (OS_HZ / VIDEO_FPS as u64);
    //     let current = get_tick();
    //     if current > target {
    //         static WARN_ONCE: atomic::AtomicBool = atomic::AtomicBool::new(false);
    //         if !WARN_ONCE.swap(true, atomic::Ordering::SeqCst) {
    //             info!("render one frame cost too long")
    //         }
    //     } else {
    //         match sleep_fn {
    //             Some(sleep) => sleep((target - current) * 1000 / OS_HZ),
    //             _ => {
    //                 while get_tick() < target {
    //                     core::hint::spin_loop();
    //                 }
    //             }
    //         }
    //     }
    //     serial_println!("frame {}", i);
    // }
}

fn write_pixel(info: &mut FrameBufferInfo, x: usize, y: usize, color: [u8; 4]) {
    let offset = info.width * y + x;
    info.buffer[offset..offset + 4].copy_from_slice(&color);
}

fn write_frame(info: &mut FrameBufferInfo, frame: &[u8; FRAME_SIZE]) {
    for y in 0..VIDEO_HEIGHT {
        for x in 0..VIDEO_WIDTH {
            let byte_index = (y * VIDEO_WIDTH + x) / 8;
            let bit_position = 7 - (x % 8);
            if (frame[byte_index] & (1 << bit_position)) != 0 {
                write_pixel(info, x, y, WHITE);
            } else {
                write_pixel(info, x, y, BLACK);
            }
        }
    }
}
