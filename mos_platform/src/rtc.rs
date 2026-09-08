//! CMOS RTC (MC146818A) wall-clock read.
//!
//! QEMU implements the standard PC RTC at ports 0x70/0x71. Values are
//! treated as UTC; there is no timezone database yet.

use x86_64::instructions::port::{PortRead, PortWrite};

const INDEX: u16 = 0x70;
const DATA: u16 = 0x71;

const REG_SECONDS: u8 = 0x00;
const REG_MINUTES: u8 = 0x02;
const REG_HOURS: u8 = 0x04;
const REG_DAY: u8 = 0x07;
const REG_MONTH: u8 = 0x08;
const REG_YEAR: u8 = 0x09;
const REG_CENTURY: u8 = 0x32;
const REG_STATUS_A: u8 = 0x0A;
const REG_STATUS_B: u8 = 0x0B;

const STATUS_A_UIP: u8 = 0x80; // update in progress
const STATUS_B_BINARY: u8 = 0x04; // registers hold binary, not BCD
const STATUS_B_24H: u8 = 0x02; // 24-hour mode

fn read_reg(reg: u8) -> u8 {
    unsafe {
        PortWrite::write_to_port(INDEX, reg | 0x80); // bit 7 disables NMI
        PortRead::read_from_port(DATA)
    }
}

fn bcd_to_bin(v: u8) -> u8 {
    (v & 0x0F) + ((v >> 4) * 10)
}

#[derive(Debug, Clone, Copy)]
pub struct RtcDateTime {
    pub year: u16,
    pub month: u8, // 1-12
    pub day: u8,   // 1-31
    pub hour: u8,  // 0-23
    pub minute: u8,
    pub second: u8,
}

/// Read the current wall-clock time from the CMOS RTC (UTC assumed).
/// Waits for an in-progress register update to finish.
pub fn read_rtc() -> RtcDateTime {
    // spin until the UIP flag clears (no update in progress)
    let mut spin = 0u32;
    while read_reg(REG_STATUS_A) & STATUS_A_UIP != 0 {
        spin += 1;
        if spin > 1_000_000 {
            break;
        }
    }

    let status_b = read_reg(REG_STATUS_B);
    let binary = status_b & STATUS_B_BINARY != 0;
    let conv = |v: u8| if binary { v } else { bcd_to_bin(v) };

    let raw_hour = read_reg(REG_HOURS);
    let mut dt = RtcDateTime {
        second: conv(read_reg(REG_SECONDS)),
        minute: conv(read_reg(REG_MINUTES)),
        hour: 0,
        day: conv(read_reg(REG_DAY)),
        month: conv(read_reg(REG_MONTH)),
        year: conv(read_reg(REG_YEAR)) as u16,
    };

    if status_b & STATUS_B_24H != 0 {
        dt.hour = conv(raw_hour);
    } else {
        // 12-hour mode: bit 7 of the raw value is the PM flag
        let pm = raw_hour & 0x80 != 0;
        dt.hour = conv(raw_hour & 0x7F);
        if pm {
            dt.hour += 12;
        }
    }

    // century register: implemented by QEMU and most chipsets
    let century = read_reg(REG_CENTURY);
    if century != 0 && century != 0xFF {
        dt.year += conv(century) as u16 * 100;
    } else {
        dt.year += 2000;
    }
    dt
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar
/// (Howard Hinnant's days_from_civil algorithm).
pub fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as i64; // [0, 399]
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) as i64 + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// Unix epoch seconds for a date/time read from the RTC.
pub fn epoch_seconds(dt: &RtcDateTime) -> u64 {
    let days = days_from_civil(dt.year as i64, dt.month as u32, dt.day as u32);
    let secs = days * 86400 + dt.hour as i64 * 3600 + dt.minute as i64 * 60 + dt.second as i64;
    secs.max(0) as u64
}
