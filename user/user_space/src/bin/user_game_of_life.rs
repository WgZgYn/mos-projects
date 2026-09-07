#![no_std]
#![no_main]

extern crate alloc;

use alloc::vec::Vec;
use user_lib::{clock_us, create_window, rand, sleep, window_mark_dirty, window_sync};

type RGBA = [u8; 4];

const WHITE: RGBA = [255, 255, 255, 255];
const BLACK: RGBA = [0, 0, 0, 255];

const WIDTH: usize = 200;
const HEIGHT: usize = 200;
const LIVE_RATE_DIV: usize = 2;
const INITIAL_LIVES: usize = WIDTH * HEIGHT / LIVE_RATE_DIV;
const FPS: usize = 15;
const FRAME_MS: u64 = 1000 / FPS as u64;

// 用 heap 分配的大 grid，避免栈溢出
struct Grid {
    data: Vec<bool>,
}
impl Grid {
    fn new() -> Self {
        let mut data = Vec::with_capacity(WIDTH * HEIGHT);
        data.resize(WIDTH * HEIGHT, false);
        Self { data }
    }
    fn get(&self, x: usize, y: usize) -> bool {
        self.data[y * WIDTH + x]
    }
    fn set(&mut self, x: usize, y: usize, v: bool) {
        self.data[y * WIDTH + x] = v;
    }
    fn swap(&mut self, other: &mut Self) {
        core::mem::swap(&mut self.data, &mut other.data);
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let info = match create_window(1600, 200, WIDTH as u64, HEIGHT as u64) {
        Some(i) => i,
        None => return -1,
    };
    let pitch = info.pitch as usize;
    let buf = unsafe {
        core::slice::from_raw_parts_mut(info.pixels_ptr as *mut RGBA, pitch * HEIGHT)
    };

    let mut current = Grid::new();
    let mut buffer = Grid::new();

    for _ in 0..INITIAL_LIVES {
        let r = rand();
        let x = (r as usize) % WIDTH;
        let y = ((r >> 32) as usize) % HEIGHT;
        current.set(x, y, true);
    }

    let mut last_frame_us = clock_us();

    loop {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let mut live = 0u8;
                for dy in y.saturating_sub(1)..=(y + 1).min(HEIGHT - 1) {
                    for dx in x.saturating_sub(1)..=(x + 1).min(WIDTH - 1) {
                        if dx == x && dy == y { continue; }
                        if current.get(dx, dy) { live += 1; }
                    }
                }
                buffer.set(x, y, matches!(
                    (current.get(x, y), live),
                    (true, 2 | 3) | (false, 3)
                ));
            }
        }
        current.swap(&mut buffer);

        for y in 0..HEIGHT {
            let row_off = y * pitch;
            for x in 0..WIDTH {
                buf[row_off + x] = if current.get(x, y) { WHITE } else { BLACK };
            }
        }

        window_mark_dirty(info.id, 0, 0, WIDTH as u64, HEIGHT as u64);
        window_sync();

        let now_us = clock_us();
        if FRAME_MS * 1000 > now_us - last_frame_us {
            sleep(FRAME_MS.saturating_sub((now_us - last_frame_us) / 1000).max(1));
        }
        last_frame_us = clock_us();
    }
}
