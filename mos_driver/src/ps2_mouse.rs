use conquer_once::spin::OnceCell;
use core::sync::atomic::{AtomicBool, Ordering};
use core::task::Waker;
use crossbeam_queue::ArrayQueue;
use futures_util::task::AtomicWaker;
use log::warn;
use mos_task::scheduler::{ParkDecision, park_current};
use x86_64::instructions::port::Port;
use crate::serial_println;

const CMD_AUX_ENABLE: u8 = 0xA8;
const CMD_READ_CONFIG: u8 = 0x20;
const CMD_WRITE_CONFIG: u8 = 0x60;
const CMD_WRITE_MOUSE: u8 = 0xD4;
const MOUSE_CMD_RESET: u8 = 0xFF;
const MOUSE_CMD_DEFAULTS: u8 = 0xF6;
const MOUSE_CMD_ENABLE_REPORT: u8 = 0xF4;
const PORT_DATA: u16 = 0x60;
const PORT_STATUS: u16 = 0x64;
const STATUS_OUT_FULL: u8 = 0x01;
const STATUS_IN_EMPTY: u8 = 0x02;
const STATUS_MOUSE_DATA: u8 = 0x20;

static MOUSE_QUEUE: OnceCell<ArrayQueue<MousePacket>> = OnceCell::uninit();
static WAKER: AtomicWaker = AtomicWaker::new();

/// 当前正在构建的数据包状态机
struct PacketBuilder {
    state: u8,
    byte0: u8,
    byte1: u8,
}

static PACKET_BUILDER: spin::Mutex<PacketBuilder> = spin::Mutex::new(PacketBuilder {
    state: 0,
    byte0: 0,
    byte1: 0,
});

#[derive(Debug, Clone, Copy, Default)]
pub struct MousePacket {
    pub dx: i16,
    pub dy: i16,
    pub left: bool,
    pub right: bool,
    pub middle: bool,
}

fn try_init_queue() {
    match MOUSE_QUEUE.try_init_once(|| ArrayQueue::new(100)) {
        Ok(_) => {}
        Err(conquer_once::TryInitError::AlreadyInit) => {
            static WARN_ONCE: AtomicBool = AtomicBool::new(false);
            if !WARN_ONCE.swap(true, Ordering::AcqRel) {
                warn!("MOUSE_QUEUE is already initialized");
            }
        }
        Err(conquer_once::TryInitError::WouldBlock) => {}
    }
}

fn has_packet() -> bool {
    MOUSE_QUEUE
        .try_get()
        .map(|q| !q.is_empty())
        .unwrap_or(false)
}

fn try_get_packet() -> Option<MousePacket> {
    MOUSE_QUEUE.try_get().ok().and_then(|q| q.pop())
}

/// 轮询检查鼠标数据（不依赖中断）- 用于诊断
/// 如果鼠标在产生数据，即使 IRQ12 不通也能读到
pub fn poll_mouse() -> Option<MousePacket> {
    use x86_64::instructions::port::Port;
    unsafe {
        let status: u8 = Port::<u8>::new(PORT_STATUS).read();
        if status & STATUS_MOUSE_DATA != 0 {
            let byte: u8 = Port::<u8>::new(PORT_DATA).read();
            add_mouse_byte(byte);
            return try_get_packet();
        }
    }
    None
}

/// 从中断处理程序调用，不能阻塞或分配
pub fn add_mouse_byte(byte: u8) {
    try_init_queue();

    let mut builder = PACKET_BUILDER.lock();

    match builder.state {
        0 => {
            // Byte 0: bit3 必须为 1（标准 PS/2 鼠标协议特征位）
            if byte & 0x08 == 0 {
                return;
            }
            builder.byte0 = byte;
            builder.state = 1;
        }
        1 => {
            builder.byte1 = byte;
            builder.state = 2;
        }
        2 => {
            let byte0 = builder.byte0;
            let byte1 = builder.byte1;
            builder.state = 0;

            drop(builder);

            let left = byte0 & 0x01 != 0;
            let right = byte0 & 0x02 != 0;
            let middle = byte0 & 0x04 != 0;

            // 符号扩展到 16 位
            let mut dx = byte1 as u16;
            if byte0 & 0x10 != 0 {
                dx |= 0xFF00;
            }
            let mut dy = byte as u16;
            if byte0 & 0x20 != 0 {
                dy |= 0xFF00;
            }

            // PS/2 Y 轴：向下为正 → 屏幕坐标 Y 向下
            // 两者方向一致，但按照惯例我们取反以使"向上移动"得到正 dy
            let dy: i16 = -(dy as i16);
            let dx: i16 = dx as i16;

            let packet = MousePacket {
                dx,
                dy,
                left,
                right,
                middle,
            };

            if let Ok(queue) = MOUSE_QUEUE.try_get()
                && queue.push(packet).is_ok()
            {
                WAKER.wake();
            }
        }
        _ => unreachable!(),
    }
}

pub struct MouseSyncStream {
    _private: (),
}

impl MouseSyncStream {
    pub fn new() -> Self {
        try_init_queue();
        Self { _private: () }
    }

    pub fn next_packet(&mut self) -> MousePacket {
        loop {
            if let Some(packet) = try_get_packet() {
                return packet;
            }
            park_current(|task| {
                if has_packet() {
                    ParkDecision::Unpark
                } else {
                    WAKER.register(&Waker::from(task.clone()));
                    ParkDecision::Park(())
                }
            })
        }
    }

    pub fn try_next_packet(&mut self) -> Option<MousePacket> {
        try_get_packet()
    }
}

impl Default for MouseSyncStream {
    fn default() -> Self {
        Self::new()
    }
}

/// PS/2 鼠标初始化 — 在中断可用后调用
/// 返回 true 表示初始化成功
///
/// 端口说明:
///   0x60 读 = 数据寄存器, 0x60 写 = 数据寄存器
///   0x64 读 = 状态寄存器, 0x64 写 = 命令寄存器
///   i8042 命令 (0x20/0x60/0xA8/0xD4) → 写端口 0x64
///   数据字节 → 写端口 0x60
pub fn init_ps2_mouse() -> bool {
    try_init_queue();

    unsafe {
        let mut cmd_port = Port::<u8>::new(PORT_STATUS);   // 0x64
        let mut data_port = Port::<u8>::new(PORT_DATA);     // 0x60

        serial_println!("[mouse] init start");

        // 1. 清空输出缓冲区中的陈旧数据
        flush_output(&mut cmd_port, &mut data_port);

        // 2. 读配置字节: 命令 0x20→0x64, 读数据←0x60
        if !wait_input_empty_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout: input not empty for read config");
            return false;
        }
        cmd_port.write(CMD_READ_CONFIG);
        if !wait_output_full_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout: no config byte");
            return false;
        }
        let config = data_port.read();
        serial_println!("[mouse] config byte = {:#x}", config);

        // 3. 写回配置: 命令 0x60→0x64, 数据→0x60
        let new_config = config | 0x03 & !0x20;
        serial_println!("[mouse] new config byte = {:#x}", new_config);

        if !wait_input_empty_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout: input not empty for write config");
            return false;
        }
        cmd_port.write(CMD_WRITE_CONFIG);
        if !wait_input_empty_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout: input not empty for config data");
            return false;
        }
        data_port.write(new_config);

        // 验证配置是否正确写入
        if !wait_input_empty_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout verifying config write");
            return false;
        }
        cmd_port.write(CMD_READ_CONFIG);
        if !wait_output_full_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout reading back config");
            return false;
        }
        let verify = data_port.read();
        serial_println!("[mouse] verify config = {:#x}", verify);
        if verify & 0x02 == 0 {
            serial_println!("[mouse] WARNING: mouse IRQ bit (bit1) not set!");
        }
        if verify & 0x20 != 0 {
            serial_println!("[mouse] WARNING: mouse clock disabled (bit5 set)!");
        }

        // 4. 启用辅助端口: 命令 0xA8→0x64
        if !wait_input_empty_to(&mut cmd_port, 100_000) {
            serial_println!("[mouse] timeout: input not empty for aux enable");
            return false;
        }
        cmd_port.write(CMD_AUX_ENABLE);

        // 5. 向鼠标发送 RESET: 先写 0xD4→0x64, 再写 0xFF→0x60
        serial_println!("[mouse] probing with RESET...");
        if !send_mouse_cmd_to(&mut cmd_port, &mut data_port, MOUSE_CMD_RESET, 100_000) {
            serial_println!("[mouse] failed to send RESET — no mouse?");
            return false;
        }

        let ack = read_response_to(&mut cmd_port, &mut data_port, 100_000);
        serial_println!("[mouse] RESET ACK = {:#x}", ack);
        if ack != 0xFA {
            serial_println!("[mouse] no ACK (0xFA), assuming no mouse present");
            return false;
        }

        let selftest = read_response_to(&mut cmd_port, &mut data_port, 200_000);
        serial_println!("[mouse] self-test = {:#x}", selftest);
        if selftest != 0xAA {
            serial_println!("[mouse] self-test failed, got {:#x}", selftest);
            return false;
        }

        let mouse_id = read_response_to(&mut cmd_port, &mut data_port, 100_000);
        serial_println!("[mouse] id = {:#x}", mouse_id);

        // 6. 设置默认值
        if !send_mouse_cmd_to(&mut cmd_port, &mut data_port, MOUSE_CMD_DEFAULTS, 100_000) {
            serial_println!("[mouse] failed to set defaults");
            return false;
        }
        let ack = read_response_to(&mut cmd_port, &mut data_port, 100_000);
        serial_println!("[mouse] defaults ACK = {:#x}", ack);

        // 7. 启用数据报告
        if !send_mouse_cmd_to(&mut cmd_port, &mut data_port, MOUSE_CMD_ENABLE_REPORT, 100_000) {
            serial_println!("[mouse] failed to enable reports");
            return false;
        }
        let ack = read_response_to(&mut cmd_port, &mut data_port, 100_000);
        serial_println!("[mouse] enable report ACK = {:#x}", ack);

        serial_println!("[mouse] init SUCCESS");
        true
    }
}

unsafe fn flush_output(cmd: &mut Port<u8>, data: &mut Port<u8>) {
    for _ in 0..32 {
        if unsafe { cmd.read() } & STATUS_OUT_FULL != 0 {
            unsafe { data.read() };
        } else {
            break;
        }
    }
}

unsafe fn wait_input_empty_to(cmd: &mut Port<u8>, max_loops: u32) -> bool {
    for _ in 0..max_loops {
        if unsafe { cmd.read() } & STATUS_IN_EMPTY == 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

unsafe fn wait_output_full_to(cmd: &mut Port<u8>, max_loops: u32) -> bool {
    for _ in 0..max_loops {
        if unsafe { cmd.read() } & STATUS_OUT_FULL != 0 {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// 给鼠标发命令: 先写 0xD4→0x64, 再写命令字节→0x60
unsafe fn send_mouse_cmd_to(
    cmd: &mut Port<u8>,
    data: &mut Port<u8>,
    mouse_cmd: u8,
    max_loops: u32,
) -> bool {
    unsafe {
        if !wait_input_empty_to(cmd, max_loops) {
            return false;
        }
        cmd.write(CMD_WRITE_MOUSE);     // ← 0xD4 写到 0x64!
        if !wait_input_empty_to(cmd, max_loops) {
            return false;
        }
        data.write(mouse_cmd);          // ← 命令字节写到 0x60
        true
    }
}

/// 读响应: 等待输出缓冲区满, 从 0x60 读一个字节
unsafe fn read_response_to(cmd: &mut Port<u8>, data: &mut Port<u8>, max_loops: u32) -> u8 {
    unsafe {
        if !wait_output_full_to(cmd, max_loops) {
            return 0;
        }
        data.read()
    }
}
