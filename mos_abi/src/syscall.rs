use num_enum::FromPrimitive;

#[derive(Debug, Copy, Clone, FromPrimitive)]
#[repr(u64)]
#[non_exhaustive]
pub enum Syscall {
    // Filesystem
    Read = 0,
    Write = 1,
    Open = 2,
    Close = 3,

    Rand = 7,

    // Memory
    Mmap = 9,
    Munmap = 11,
    Brk = 12,

    Pipe = 22,

    // schedule
    Yield = 24,

    // Process
    Clone = 56,
    Fork = 57,
    Execve = 59,
    Exit = 60,
    Wait = 61,
    Sleep = 62,

    // inode
    GetDents = 78,
    GetCWD = 79,
    ChDir = 80,

    Mkdir = 83,
    Mkfifo = 132,
    #[num_enum(default)]
    Unknown,
}
