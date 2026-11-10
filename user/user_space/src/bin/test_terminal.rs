#![no_std]
#![no_main]
use user_lib::mos::fs::{OPOST, TIOCGWINSZ, Termios, Winsize};
use user_lib::{STDIN, STDOUT, ioctl, println, tcgetattr, tcsetattr, write};
fn out(bytes: &[u8]) {
    assert_eq!(write(STDOUT, bytes).unwrap(), bytes.len());
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut size = Winsize::default();
    assert_eq!(ioctl(STDIN, TIOCGWINSZ, &mut size as *mut _ as u64), 0);
    assert!(size.ws_row > 0 && size.ws_col > 0);
    // Each piece is a separate syscall, including incomplete UTF-8 and CSI.
    out(b"\x1b[0m\x1b[2J\x1b[H");
    println!(
        "MOS terminal / {} columns x {} rows",
        size.ws_col, size.ws_row
    );
    out(b"\x1b[3");
    out(b"1mRED ");
    out(b"\x1b[32mGREEN \x1b[34mBLUE\x1b[0m\n");
    out(b"\x1b[97;44m white on blue \x1b[0m default\n");
    out(b"UTF-8 split: ");
    out(&[0xc3]);
    out(&[0xa9]);
    out(b" / invalid: ");
    out(&[0xff]);
    out(b"\n");
    out(b"overwrite: bad\r\x1b[11Cgood\x1b[K\n");
    out(b"edit: ABCDE\x1b[3D\x1b[P\x1b[@X\n");
    let mut original = Termios::default();
    assert_eq!(tcgetattr(STDIN, &mut original), 0);
    let mut raw = original;
    raw.c_oflag &= !OPOST;
    assert_eq!(tcsetattr(STDIN, &raw), 0);
    out(b"raw line A\nB\r\n");
    assert_eq!(tcsetattr(STDIN, &original), 0);
    out(b"\x1b[38;5;208m256 color \x1b[38;2;120;200;255mRGB color\x1b[0m\n");
    let window = user_lib::create_window(0, 0, 420, 60).unwrap();
    for i in 0..window.width * window.height {
        unsafe {
            (window.pixels_ptr as *mut u32)
                .add(i as usize)
                .write_volatile(u32::from_le_bytes([160, 0, 160, 255]));
        }
    }
    user_lib::window_mark_dirty(window.id, 0, 0, window.width, window.height);
    user_lib::window_sync();
    // Update covered text. The window must stay on top until destroy, and the
    // new text must be reconstructed from cells when the window is removed.
    out(b"\x1b[s\x1b[HMOS terminal / restored after window close\x1b[K\x1b[u");
    println!("[terminal-layer] covered; press Enter to close window");
    let mut byte = [0];
    while user_lib::read(STDIN, &mut byte).unwrap() != 0 && byte[0] != b'\n' {}
    user_lib::destroy_window(window.id);
    user_lib::window_sync();
    println!("[terminal] PASS");
    0
}
