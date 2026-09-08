#[derive(Clone, Copy, Eq, PartialEq, Debug)]
pub struct FileDesc(pub usize);

/// struct stat — identical layout to Linux x86_64 (asm-generic), 144 bytes.
/// musl/glibc parse this memory layout directly.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: i64,
    pub st_mtime: i64,
    pub st_mtime_nsec: i64,
    pub st_ctime: i64,
    pub st_ctime_nsec: i64,
    pub __unused: [i64; 3],
}

// S_IFMT / S_IF* mode bits, Linux values
pub const S_IFMT: u32 = 0o170000;
pub const S_IFDIR: u32 = 0o040000;
pub const S_IFREG: u32 = 0o100000;
pub const S_IFCHR: u32 = 0o020000;
pub const S_IFBLK: u32 = 0o060000;
pub const S_IFIFO: u32 = 0o010000;
pub const S_IFLNK: u32 = 0o120000;

// default permission bits (no real user system yet)
pub const DIR_PERM: u32 = 0o755;
pub const FILE_PERM: u32 = 0o644;

// Linux d_type values for getdents64
pub const DT_UNKNOWN: u8 = 0;
pub const DT_FIFO: u8 = 1;
pub const DT_CHR: u8 = 2;
pub const DT_DIR: u8 = 4;
pub const DT_BLK: u8 = 6;
pub const DT_REG: u8 = 8;
pub const DT_LNK: u8 = 10;

/// A record in a getdents64(2) buffer, Linux dirent64 layout. The kernel
/// packs these back-to-back into the caller's buffer; d_name is a
/// NUL-terminated C string of `d_reclen - 19` bytes (flexible array).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Dirent64 {
    pub d_ino: u64,
    pub d_off: i64,
    pub d_reclen: u16,
    pub d_type: u8,
    pub d_name: [u8; 0],
}

pub const DIRENT64_HEADER: usize = 19;

/// lseek(2) whence values
pub const SEEK_SET: u32 = 0;
pub const SEEK_CUR: u32 = 1;
pub const SEEK_END: u32 = 2;

// Linux O_* open(2) flag values (x86_64). Note O_RDONLY == 0, so access
// mode cannot be tested as a bit: `flags & 3` selects READ/WRITE/RDWR.
pub const O_RDONLY: u32 = 0;
pub const O_WRONLY: u32 = 1;
pub const O_RDWR: u32 = 2;
pub const O_CREAT: u32 = 0x40;
pub const O_EXCL: u32 = 0x80;
pub const O_TRUNC: u32 = 0x200;
pub const O_APPEND: u32 = 0x400;
pub const O_NONBLOCK: u32 = 0x800;
pub const O_DIRECTORY: u32 = 0x10000;
