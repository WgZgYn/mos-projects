//! clock_gettime(2) / gettimeofday(2) implementation.
//!
//! CLOCK_REALTIME comes from the CMOS RTC (treated as UTC), CLOCK_MONOTONIC
//! from the platform timer. Both are queried on demand — no wall-clock
//! caching yet, which is fine at syscall frequency.

use mos_abi::error::KError;
use mos_abi::time::{
    CLOCK_BOOTTIME, CLOCK_MONOTONIC, CLOCK_MONOTONIC_COARSE, CLOCK_REALTIME, CLOCK_REALTIME_COARSE,
    Timespec, Timeval, Timezone,
};
use mos_arch::structs::SyscallContext;
use mos_platform::rtc::{epoch_seconds, read_rtc};
use mos_platform::timer::monotonic_time_ms;

/// Current wall-clock time as (epoch seconds, nsec) — nsec is always 0
/// because the RTC has 1s granularity.
pub fn wall_clock() -> (i64, i64) {
    (epoch_seconds(&read_rtc()) as i64, 0)
}

pub fn sys_clock_gettime(clk_id: u32) -> Result<Timespec, KError> {
    let (sec, nsec) = match clk_id {
        CLOCK_REALTIME | CLOCK_REALTIME_COARSE => wall_clock(),
        CLOCK_MONOTONIC | CLOCK_MONOTONIC_COARSE | CLOCK_BOOTTIME => {
            let ms = monotonic_time_ms();
            ((ms / 1000) as i64, ((ms % 1000) * 1_000_000) as i64)
        }
        _ => return Err(KError::InvalidArg), // EINVAL
    };
    Ok(Timespec {
        tv_sec: sec,
        tv_nsec: nsec,
    })
}

pub fn raw_sys_clock_gettime(context: &mut SyscallContext, args: [u64; 6]) {
    let ptr = args[1] as *mut Timespec;
    match sys_clock_gettime(args[0] as u32) {
        Ok(ts) => {
            if !ptr.is_null() {
                unsafe { ptr.write_volatile(ts) };
            }
            context.scratch.rax = 0;
        }
        Err(e) => crate::ret_err(context, e),
    }
}

pub fn raw_sys_gettimeofday(context: &mut SyscallContext, args: [u64; 6]) {
    let tv = args[0] as *mut Timeval;
    let tz = args[1] as *mut Timezone;
    let (sec, _) = wall_clock();
    if !tv.is_null() {
        unsafe { tv.write_volatile(Timeval { tv_sec: sec, tv_usec: 0 }) };
    }
    if !tz.is_null() {
        unsafe { tz.write_volatile(Timezone::default()) };
    }
    context.scratch.rax = 0;
}
