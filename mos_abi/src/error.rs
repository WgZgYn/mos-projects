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
    BadFd = 42,
    NoChild = 43,
    NoExec = 44,
    Interrupted = 45,
    OperationNotPermitted = 46,
    NotTty = 47,
}

impl KError {
    /// Linux x86_64 errno, so that a musl/glibc program can interpret the
    /// negative return value of a failed syscall directly.
    pub const fn to_errno(self) -> i64 {
        match self {
            Self::None => 0,
            Self::InvalidArg => 22,       // EINVAL
            Self::OutOfMem => 12,         // ENOMEM
            Self::Overflow => 75,         // EOVERFLOW
            Self::NotMapped => 12,        // ENOMEM
            Self::AlreadyMapped => 17,    // EEXIST
            Self::FrameAllocFailed => 12, // ENOMEM
            Self::OutOfBounds => 14,      // EFAULT
            Self::NoProcess => 3,         // ESRCH
            Self::NoThread => 3,          // ESRCH
            Self::StillRunning => 11,     // EAGAIN
            Self::NoFile => 2,            // ENOENT
            Self::NotDir => 20,           // ENOTDIR
            Self::IsDir => 21,            // EISDIR
            Self::NoPermission => 13,     // EACCES
            Self::FileTooLarge => 27,     // EFBIG
            Self::PipeClosed => 32,       // EPIPE
            Self::NoDevice => 19,         // ENODEV
            Self::Unsupported => 38,      // ENOSYS
            Self::UnknownSyscall => 38,   // ENOSYS
            Self::BadFd => 9,             // EBADF
            Self::NoChild => 10,          // ECHILD
            Self::NoExec => 8,            // ENOEXEC
            Self::Interrupted => 4,       // EINTR
            Self::OperationNotPermitted => 1, // EPERM
            Self::NotTty => 25,           // ENOTTY
        }
    }
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
            Self::BadFd => "bad file descriptor",
            Self::NoChild => "no child processes",
            Self::NoExec => "exec format error",
            Self::Interrupted => "interrupted system call",
            Self::OperationNotPermitted => "operation not permitted",
            Self::NotTty => "not a tty",
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
