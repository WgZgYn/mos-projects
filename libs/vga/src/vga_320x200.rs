use vga::writers::{GraphicsWriter, Screen};

static GRAPHICS: vga::writers::Graphics320x200x256 = vga::writers::Graphics320x200x256::new();

const VGA_VIDEO_HEIGHT: usize = vga::writers::Graphics320x200x256::HEIGHT;
const VGA_VIDEO_WIDTH: usize = vga::writers::Graphics320x200x256::WIDTH;

pub fn test_bad_apple(get_tick: fn() -> usize, sleep: fn(usize)) {
    fn write_binary(frame: &[u8; VGA_VIDEO_HEIGHT * VGA_VIDEO_WIDTH / 8]) {
        for y in 0..VGA_VIDEO_HEIGHT {
            for x in 0..VGA_VIDEO_WIDTH {
                let byte_index = (y * VGA_VIDEO_WIDTH + x) / 8;
                let bit_position = 7 - (x % 8);
                if (frame[byte_index] & (1 << bit_position)) != 0 {
                    GRAPHICS.set_pixel(x, y, 0x0F);
                } else {
                    GRAPHICS.set_pixel(x, y, 0x00);
                }
            }
        }
    }

    let bs = include_bytes!("video_320x200_1bpp.bin");
    for (i, frame) in bs
        .as_chunks::<{ VGA_VIDEO_HEIGHT * VGA_VIDEO_WIDTH / 8 }>()
        .0
        .iter()
        .take(5)
        .enumerate()
    {
        let start = get_tick();
        write_binary(frame);
        let end = get_tick();
        if end - start < 1000 / 17 {
            sleep(1000 / 17 - (end - start));
        }
    }
}

pub fn init() {
    GRAPHICS.set_mode()
}
