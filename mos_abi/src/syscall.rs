use num_enum::FromPrimitive;

/// Syscall numbers follow the Linux x86_64 ABI table so that a statically
/// linked musl (or glibc) binary can call the kernel directly.
///
/// MOS-specific syscalls (semaphore / window / display / custom clock) live
/// in the private range starting at [`MOS_SYSCALL_BASE`], far away from any
/// number Linux has ever used.
#[derive(Debug, Copy, Clone, FromPrimitive)]
#[repr(u64)]
#[non_exhaustive]
pub enum Syscall {
    // ===== Linux x86_64 numbers =====
    // Filesystem
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,
    Stat = 4,
    Fstat = 5,
    Lstat = 6,
    Lseek = 8,

    // Memory
    Mmap = 9,
    Mprotect = 10,
    Munmap = 11,
    Brk = 12,
    Madvise = 28,

    // Pipe / dup
    Pipe = 22,
    Readv = 19,
    Writev = 20,
    Dup = 32,
    DupTo = 33,
    Fcntl = 72,
    Dup3 = 292,

    // Time
    Nanosleep = 35,
    GetTimeOfDay = 96,

    // Process
    GetPid = 39,
    Clone = 56,
    Fork = 57,
    Execve = 59,
    Exit = 60,
    Wait = 61,
    Uname = 63,
    Getuid = 102,
    Getgid = 104,
    Geteuid = 107,
    Getegid = 108,
    GetPpid = 110,
    Mkfifo = 132,
    ArchPrctl = 158,
    SetTidAddress = 218,
    ClockGetTime = 228,
    ExitGroup = 231,

    // schedule
    Yield = 24,

    // inode
    GetDents64 = 217,
    GetCWD = 79,
    ChDir = 80,
    Mkdir = 83,
    Unlink = 87,
    Rmdir = 84,
    Rename = 82,
    Link = 86,
    Symlink = 88,
    Readlink = 89,
    Access = 21,
    Umask = 95,
    Truncate = 76,
    Ftruncate = 77,
    Chmod = 90,
    Fchmod = 91,
    Faccessat = 269,

    // ===== MOS private syscalls =====
    Rand = MOS_SYSCALL_BASE,
    GetCpu,
    CreateSemaphore,
    SignalSemaphore,
    WaitSemaphore,
    CreateThread,
    CreateWindow,
    WindowMarkDirty,
    WindowSync,
    DestroyWindow,
    ClockTicks,
    ClockFreq,

    #[num_enum(default)]
    Unknown,
}

/// Base number for MOS-private syscalls. Linux x86_64 currently uses numbers
/// up to ~460, so 1000+ never collides with a future Linux syscall.
pub const MOS_SYSCALL_BASE: u64 = 1000;
