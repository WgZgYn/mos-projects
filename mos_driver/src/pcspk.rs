use core::arch::asm;
use core::sync::atomic::{AtomicBool, Ordering};

const PIT_FREQ: u32 = 1193182;

pub fn speaker_on(freq: u32) {
    if freq == 0 {
        speaker_off();
        return;
    }
    let divisor: u16 = (PIT_FREQ / freq) as u16;

    static CONFIG_ONCE: AtomicBool = AtomicBool::new(false);

    unsafe {
        if !CONFIG_ONCE.swap(true, Ordering::SeqCst) {
            // 设置 PIT 通道2，方波模式
            asm!("out dx, al",
            in("dx") 0x43u16,
            in("al") 0b10110110u8,
            );
        }

        // 发送低字节
        asm!("out dx, al",
        in("dx") 0x42u16,
        in("al") (divisor & 0xff) as u8,
        );

        // 发送高字节
        asm!("out dx, al",
        in("dx") 0x42u16,
        in("al") (divisor >> 8) as u8,
        );

        // 打开 speaker gate
        let mut tmp: u8;
        asm!("in al, dx",
        in("dx") 0x61u16,
        out("al") tmp,
        );

        tmp |= 3;

        asm!("out dx, al",
        in("dx") 0x61u16,
        in("al") tmp,
        );
    }
}

pub fn speaker_off() {
    unsafe {
        let mut tmp: u8;
        asm!("in al, dx",
        in("dx") 0x61u16,
        out("al") tmp,
        );

        tmp &= 0xFC;

        asm!("out dx, al",
        in("dx") 0x61u16,
        in("al") tmp,
        );
    }
}
