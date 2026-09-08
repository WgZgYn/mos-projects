//! ABI structs that must match the Linux x86_64 layouts, so musl/glibc
//! binaries can pass them across the syscall boundary.

// clock_gettime(2) clock ids (Linux)
pub const CLOCK_REALTIME: u32 = 0;
pub const CLOCK_MONOTONIC: u32 = 1;
pub const CLOCK_PROCESS_CPUTIME_ID: u32 = 2;
pub const CLOCK_THREAD_CPUTIME_ID: u32 = 3;
pub const CLOCK_MONOTONIC_RAW: u32 = 4;
pub const CLOCK_REALTIME_COARSE: u32 = 5;
pub const CLOCK_MONOTONIC_COARSE: u32 = 6;
pub const CLOCK_BOOTTIME: u32 = 7;

/// struct timespec — identical layout to Linux x86_64.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

/// struct timeval — identical layout to Linux x86_64.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

/// struct timezone (legacy, kept for gettimeofday(2) ABI completeness).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Timezone {
    pub tz_minuteswest: i32,
    pub tz_dsttime: i32,
}

/// struct utsname — identical layout to Linux x86_64 (6 × 65 bytes).
#[repr(C)]
pub struct Utsname {
    pub sysname: [u8; 65],
    pub nodename: [u8; 65],
    pub release: [u8; 65],
    pub version: [u8; 65],
    pub machine: [u8; 65],
    pub domainname: [u8; 65],
}

impl Utsname {
    pub fn new_filled(
        sysname: &str,
        nodename: &str,
        release: &str,
        version: &str,
        machine: &str,
        domainname: &str,
    ) -> Self {
        let mut u = Utsname {
            sysname: [0; 65],
            nodename: [0; 65],
            release: [0; 65],
            version: [0; 65],
            machine: [0; 65],
            domainname: [0; 65],
        };
        u.sysname[..sysname.len()].copy_from_slice(sysname.as_bytes());
        u.nodename[..nodename.len()].copy_from_slice(nodename.as_bytes());
        u.release[..release.len()].copy_from_slice(release.as_bytes());
        u.version[..version.len()].copy_from_slice(version.as_bytes());
        u.machine[..machine.len()].copy_from_slice(machine.as_bytes());
        u.domainname[..domainname.len()].copy_from_slice(domainname.as_bytes());
        u
    }
}

impl Default for Utsname {
    fn default() -> Self {
        Utsname::new_filled("MOS", "mos", "0.1.0", "dev", "x86_64", "")
    }
}
