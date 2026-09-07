/// 内核统一错误码
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i64)]
pub enum KError {
    None = 0,
    InvalidArg = 1,
    OutOfMem = 2,
    Overflow = 3,
    NotMapped = 10,
    AlreadyMapped = 11,
    FrameAllocFailed = 12,
    OutOfBounds = 13,
    NoProcess = 20,
    NoThread = 21,
    StillRunning = 22,
    NoFile = 30,
    NotDir = 31,
    IsDir = 32,
    NoPermission = 33,
    FileTooLarge = 34,
    PipeClosed = 35,
    NoDevice = 40,
    Unsupported = 41,
    UnknownSyscall = 50,
}

pub type KResult<T> = Result<T, KError>;

impl KError {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::None => "no error",
            Self::InvalidArg => "invalid argument",
            Self::OutOfMem => "out of memory",
            Self::Overflow => "overflow",
            Self::NotMapped => "not mapped",
            Self::AlreadyMapped => "already mapped",
            Self::FrameAllocFailed => "frame allocation failed",
            Self::OutOfBounds => "out of bounds",
            Self::NoProcess => "no such process",
            Self::NoThread => "no such thread",
            Self::StillRunning => "still running",
            Self::NoFile => "no such file",
            Self::NotDir => "not a directory",
            Self::IsDir => "is a directory",
            Self::NoPermission => "permission denied",
            Self::FileTooLarge => "file too large",
            Self::PipeClosed => "pipe closed",
            Self::NoDevice => "no such device",
            Self::Unsupported => "not supported",
            Self::UnknownSyscall => "unknown syscall",
        }
    }
}

impl From<&str> for KError {
    fn from(_s: &str) -> Self {
        KError::Unsupported
    }
}

impl core::fmt::Display for KError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}
