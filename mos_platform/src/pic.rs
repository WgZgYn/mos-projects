#![allow(unused)]

use pic8259::ChainedPics;
use crate::InterruptController;

const PIC_1_OFFSET: u8 = 32;
const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// PIT 相关端口和常量
const PIT_CHANNEL0: u16 = 0x40; // 通道0数据端口
const PIT_CHANNEL1: u16 = 0x41; // 通道1数据端口
const PIT_CHANNEL2: u16 = 0x42; // 通道2数据端口
const PIT_COMMAND: u16 = 0x43; // 命令寄存器端口
const PIT0_INTERVAL_MS: u64 = 1;
const PIT1_INTERVAL_MS: u64 = 10;

// PIT 基础频率
const PIT_BASE_FREQUENCY: u32 = 1_193_182; // 1.193182 MHz

// 配置命令字节：
// Bits 7-6: 通道选择 (00 = 通道0)
// Bits 5-4: 访问模式 (11 = 先低字节后高字节)
// Bits 3-1: 操作模式 (010 = 速率发生器)
// Bit 0:    BCD模式 (0 = 16位二进制)
fn configure_pit_interval() {
    unsafe fn config(chan: u8, interval: u64) {
        unsafe {
            use x86_64::instructions::port::PortWrite;
            // 计算分频值：frequency = base_frequency / divider
            let frequency_hz: u64 = 1000 / interval; // 转换为Hz
            let divider: u16 = (PIT_BASE_FREQUENCY as u64 / frequency_hz) as u16;

            let command_byte: u8 = 0b00_11_010_0 | (chan & 3 << 6); // 0x34
            // 发送配置命令
            PortWrite::write_to_port(PIT_COMMAND, command_byte);

            // 写入分频值（先低字节，后高字节）
            PortWrite::write_to_port(PIT_CHANNEL0, (divider & 0xFF) as u8);
            PortWrite::write_to_port(PIT_CHANNEL0, ((divider >> 8) & 0xFF) as u8);
            // serial_println!("config chan {}, interval {}", chan, interval);
        }
    }

    // 将0通道设置为1ms时钟间隔
    unsafe {
        config(0, PIT0_INTERVAL_MS);
    }
}

pub fn init_timer() {
    configure_pit_interval()
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
}

impl InterruptIndex {
    pub const fn as_u8(self) -> u8 {
        self as u8
    }
    pub const fn as_usize(self) -> usize {
        self.as_u8() as _
    }
}

static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

pub struct PIC8259;
impl InterruptController for PIC8259 {
    fn initialize(&self) {
        unsafe { PICS.lock().initialize() }
    }

    fn enable_irq(&self, irq: u8) {
        if irq > 15 {
            return;
        }
        let mut masks = unsafe { PICS.lock().read_masks() };
        if irq < 8 {
            masks[0] |= 1 << irq;
        } else {
            masks[1] |= 1 << (irq - 8);
        }
        unsafe {
            PICS.lock().write_masks(masks[0], masks[1]);
        }
    }

    fn disable_irq(&self, irq: u8) {
        if irq > 15 {
            return;
        }
        let mut masks = unsafe { PICS.lock().read_masks() };
        if irq < 8 {
            masks[0] &= !(1 << irq);
        } else {
            masks[1] &= !(1 << (irq - 8));
        }
        unsafe {
            PICS.lock().write_masks(masks[0], masks[1]);
        }
    }

    fn eoi(&self, irq: u8) {
        unsafe { PICS.lock().notify_end_of_interrupt(irq) }
    }
}

pub fn pic8059_mode() -> PIC8259 {
    PIC8259
}
