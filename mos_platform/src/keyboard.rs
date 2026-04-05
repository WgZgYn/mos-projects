use log::info;
use x86_64::instructions::port::Port;

pub fn clear_keyboard_buffer() {
    const STATE_PORT: u16 = 0x64;
    const DATA_PORT: u16 = 0x60;

    let mut status = Port::<u8>::new(STATE_PORT);
    let mut data = Port::<u8>::new(DATA_PORT);
    let mut n = 0;
    let mut limit = 1024; // 上限

    unsafe {
        while limit > 0 {
            if (status.read() & 0x01) != 0 {
                let _ = data.read();
                n += 1;
            } else {
                break;
            }
            limit -= 1;
        }
    }

    if limit == 0 {
        info!("keyboard buffer clear aborted (possible broken controller)");
    }

    if n != 0 {
        info!("drop {} stale bytes", n);
    }
}
