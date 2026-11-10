//! Linux x86_64 wait status subset.
//! Reference: https://man7.org/linux/man-pages/man2/waitpid.2.html

pub const WNOHANG: u32 = 1;
pub const WUNTRACED: u32 = 2;

/// Normal exit exposes only the low eight bits of the exit code.
pub const fn exit_status(code: i32) -> i32 {
    (code & 0xff) << 8
}

pub const fn exited(status: i32) -> bool {
    status & 0x7f == 0
}

pub const fn exit_code(status: i32) -> i32 {
    (status >> 8) & 0xff
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linux_exit_status_truncates_and_distinguishes_signals() {
        for (code, expected) in [(0, 0), (42, 0x2a00), (256, 0), (-1, 0xff00)] {
            assert_eq!(exit_status(code), expected);
            assert!(exited(expected));
            assert_eq!(exit_code(expected), code & 255);
        }
        assert!(!exited(9));
        assert!(!exited(0x7f));
    }
}
