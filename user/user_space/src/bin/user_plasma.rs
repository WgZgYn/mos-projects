#![no_std]
#![no_main]

use core::f64::consts::PI;
use user_lib::{clock_us, create_window, destroy_window, exit, window_mark_dirty, window_sync};

type RGBA = [u8; 4];

const W: usize = 480;
const H: usize = 360;
const FPS_TARGET_US: u64 = 33_333; // ~30 FPS

// ─── 简易数学 (no_std 环境下 libm::sin 可用) ───

fn sin(x: f64) -> f64 {
    libm::sin(x)
}
fn cos(x: f64) -> f64 {
    libm::cos(x)
}
fn sqrt(x: f64) -> f64 {
    libm::sqrt(x)
}
fn floor(x: f64) -> f64 {
    libm::floor(x)
}
fn fract(x: f64) -> f64 {
    x - floor(x)
}

// ─── HSL→RGB ──────────────────────────

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> RGBA {
    let c = (1.0 - libm::fabs(2.0 * l - 1.0)) * s;
    let hp = h * 6.0;
    let x = c * (1.0 - libm::fabs(hp % 2.0 - 1.0));
    let m = l - c / 2.0;

    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };

    [
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
        255,
    ]
}

// ─── Plasma 核心 ────────────────────────

/// 计算 (x, y) 处的 plasma 值 → 映射到色相
fn plasma_value(px: usize, py: usize, t: f64) -> f64 {
    let fx = px as f64 / W as f64;
    let fy = py as f64 / H as f64;
    let cx = fx - 0.5;
    let cy = fy - 0.5;
    let dist = sqrt(cx * cx + cy * cy);

    let v = sin(fx * 8.0 + t)
        + sin(sin(fy * 6.0 + t * 0.7) * 3.0 + t * 0.5)
        + sin(dist * 12.0 - t * 1.3)
        + sin((fx + fy) * 10.0 + t * 0.6)
        + cos(fx * 4.0 - fy * 4.0 + t * 0.9);

    // 归一化到 [0, 1)
    (v + 5.0) / 10.0
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let info = match create_window(100, 100, W as u64, H as u64) {
        Some(i) => i,
        None => return -1,
    };
    let pitch = info.pitch as usize;
    let buf =
        unsafe { core::slice::from_raw_parts_mut(info.pixels_ptr as *mut RGBA, pitch * H) };

    let start_t = clock_us();
    let mut frame = 0u64;
    let mut last_fps_print = start_t;

    loop {
        let now = clock_us();
        let t = (now - start_t) as f64 / 1_000_000.0; // 秒为单位

        // 逐像素计算 plasma
        for y in 0..H {
            let row_off = y * pitch;
            for x in 0..W {
                let v = plasma_value(x, y, t);
                // 色相随时间旋转 + 每个像素的 plasma 值决定基础色相
                let hue = fract(v + t * 0.05);
                let sat = 0.7 + 0.3 * sin(v * PI);
                let lum = 0.3 + 0.4 * v;
                buf[row_off + x] = hsl_to_rgb(hue, sat, lum);
            }
        }

        window_mark_dirty(info.id, 0, 0, W as u64, H as u64);
        window_sync();

        frame += 1;

        // FPS 统计
        if now - last_fps_print >= 5_000_000 {
            let elapsed = (now - last_fps_print) as f64 / 1_000_000.0;
            let fps = frame as f64 / elapsed;
            // 在窗口左上角画一个 FPS 指示色块 (绿色=流畅, 红色=卡顿)
            let status_color: RGBA = if fps > 25.0 {
                [0, 255, 0, 255]
            } else if fps > 15.0 {
                [255, 255, 0, 255]
            } else {
                [255, 0, 0, 255]
            };
            for dy in 0..6 {
                let ro = dy * pitch;
                for dx in 0..(fps as usize).min(50) * 2 {
                    buf[ro + dx] = status_color;
                }
            }
            frame = 0;
            last_fps_print = now;
        }

        // 帧率控制
        let elapsed_us = clock_us() - now;
        if elapsed_us < FPS_TARGET_US {
            let target = now + FPS_TARGET_US;
            while clock_us() < target {
                core::hint::spin_loop();
            }
        }
    }
}
