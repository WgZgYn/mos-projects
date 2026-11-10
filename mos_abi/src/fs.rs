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

// ---- termios / ioctl ----

/// Linux struct termios (x86_64, glibc/musl layout: 36 bytes, no padding)
#[repr(C)]
#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; 19],
}

impl Termios {
    pub const BYTE_LEN: usize = 36;

    /// Decode the UAPI representation without creating an aligned reference
    /// to user memory. The Linux x86_64 structure contains no padding.
    pub fn from_bytes(bytes: [u8; Self::BYTE_LEN]) -> Self {
        Self {
            c_iflag: u32::from_ne_bytes(bytes[0..4].try_into().unwrap()),
            c_oflag: u32::from_ne_bytes(bytes[4..8].try_into().unwrap()),
            c_cflag: u32::from_ne_bytes(bytes[8..12].try_into().unwrap()),
            c_lflag: u32::from_ne_bytes(bytes[12..16].try_into().unwrap()),
            c_line: bytes[16],
            c_cc: bytes[17..].try_into().unwrap(),
        }
    }

    pub fn to_bytes(self) -> [u8; Self::BYTE_LEN] {
        let mut bytes = [0; Self::BYTE_LEN];
        bytes[0..4].copy_from_slice(&self.c_iflag.to_ne_bytes());
        bytes[4..8].copy_from_slice(&self.c_oflag.to_ne_bytes());
        bytes[8..12].copy_from_slice(&self.c_cflag.to_ne_bytes());
        bytes[12..16].copy_from_slice(&self.c_lflag.to_ne_bytes());
        bytes[16] = self.c_line;
        bytes[17..].copy_from_slice(&self.c_cc);
        bytes
    }
}

/// Linux struct winsize
#[repr(C)]
#[derive(Default, Clone, Copy, Debug)]
pub struct Winsize {
    pub ws_row: u16,
    pub ws_col: u16,
    pub ws_xpixel: u16,
    pub ws_ypixel: u16,
}

// ioctl request numbers (x86_64)
pub const TCGETS: u32 = 0x5401;
pub const TCSETS: u32 = 0x5402;
pub const TCSETSW: u32 = 0x5403;
pub const TCSETSF: u32 = 0x5404;
pub const TIOCSCTTY: u32 = 0x540e;
pub const TIOCGPGRP: u32 = 0x540f;
pub const TIOCSPGRP: u32 = 0x5410;
pub const TIOCGWINSZ: u32 = 0x5413;
pub const TIOCGSID: u32 = 0x5429;

// termios flags: canonical input with echo + signals, matching a normal tty
pub const TTY_IFLAG: u32 = 0x2501; // ICRNL | IXON
pub const TTY_OFLAG: u32 = 0x5; // OPOST | ONLCR
pub const TTY_CFLAG: u32 = 0xbf; // B38400 | CS8 | CREAD
pub const TTY_LFLAG: u32 = 0x8a3b; // ISIG | ICANON | ECHO | ECHOE | ECHOK | IEXTEN
pub const ICRNL: u32 = 0x100;
pub const OPOST: u32 = 0x1;
pub const ONLCR: u32 = 0x4;
pub const ISIG: u32 = 0x1;
pub const ICANON: u32 = 0x2;
pub const ECHO: u32 = 0x8;
pub const ECHOE: u32 = 0x10;
pub const ECHOK: u32 = 0x20;
pub const ECHONL: u32 = 0x40;
pub const NOFLSH: u32 = 0x80;
pub const VINTR: usize = 0;
pub const VERASE: usize = 2;
pub const VKILL: usize = 3;
pub const VEOF: usize = 4;
pub const VTIME: usize = 5;
pub const VMIN: usize = 6;

pub const fn default_termios() -> Termios {
    let mut control = [0; 19];
    control[VINTR] = 3;       // Ctrl-C
    control[1] = 28;          // Ctrl-\\
    control[VERASE] = 8;      // pc-keyboard reports Backspace as BS
    control[VKILL] = 21;      // Ctrl-U
    control[VEOF] = 4;        // Ctrl-D
    control[VMIN] = 1;
    control[8] = 17;          // Ctrl-Q
    control[9] = 19;          // Ctrl-S
    control[10] = 26;         // Ctrl-Z
    Termios { c_iflag: TTY_IFLAG, c_oflag: TTY_OFLAG, c_cflag: TTY_CFLAG,
        c_lflag: TTY_LFLAG, c_line: 0, c_cc: control }
}
pub const O_DIRECTORY: u32 = 0x10000;

#[cfg(test)]
mod termios_tests {
    use super::*;

    #[test]
    fn linux_termios_layout_and_roundtrip() {
        assert_eq!(size_of::<Termios>(), Termios::BYTE_LEN);
        assert_eq!(core::mem::offset_of!(Termios, c_cc), 17);
        let termios = default_termios();
        assert_eq!(Termios::from_bytes(termios.to_bytes()), termios);
        assert_eq!(termios.c_cc[VINTR], 3);
    }
}
