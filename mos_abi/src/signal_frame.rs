//! Linux x86_64 rt_sigframe byte layout (legacy FXSAVE, no XSAVE extension).
//! Sources: Linux arch/x86/include/asm/sigframe.h and uapi/asm/sigcontext.h.
//! Byte offsets avoid Rust padding and unaligned references into user memory.
pub const FRAME_BYTES: usize = 440;
pub const UCONTEXT: usize = 8;
pub const MCONTEXT: usize = 48;
pub const SIGMASK: usize = 304;
pub const SIGINFO: usize = 312;
pub const FP_POINTER: usize = MCONTEXT + 23 * 8;
pub const USER_LIMIT: u64 = 0x0000_8000_0000_0000;

pub fn read_word(bytes: &[u8], offset: usize) -> u64 {
    u64::from_ne_bytes(bytes[offset..offset + 8].try_into().unwrap())
}
pub fn write_word(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
}
pub const fn user_address(addr: u64) -> bool { addr != 0 && addr < USER_LIMIT }

/// Skip the SysV red zone, place aligned FXSAVE bytes above the frame, and
/// enter the handler with RSP % 16 == 8, exactly like a normal function call.
pub fn frame_addresses(rsp: u64) -> Option<(u64, u64)> {
    if !user_address(rsp) { return None; }
    let fp = rsp.checked_sub(128 + 512)? & !15;
    let frame = (fp.checked_sub(FRAME_BYTES as u64)? & !15).checked_sub(8)?;
    user_address(frame).then_some((frame, fp))
}

/// User flags only. IF and reserved bit 1 are forced on; privileged/control
/// flags (IOPL, NT, VM, TF, RF) cannot be injected into kernel return state.
pub const fn restored_flags(flags: u64) -> u64 {
    (flags & 0x0020_0cd5) | 0x202
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_layout_and_red_zone() {
        assert_eq!(MCONTEXT, UCONTEXT + 40);
        assert_eq!(SIGMASK, MCONTEXT + 256);
        assert_eq!(SIGINFO, SIGMASK + 8);
        assert_eq!(FRAME_BYTES, SIGINFO + 128);
        for offset in 0..16 {
            let rsp = 0x8000 + offset;
            let (frame, fp) = frame_addresses(rsp).unwrap();
            assert_eq!(frame % 16, 8);
            assert_eq!(fp % 16, 0);
            assert!(frame + FRAME_BYTES as u64 <= fp);
            assert!(fp + 512 <= rsp - 128);
        }
        assert_eq!(frame_addresses(100), None);
        assert_eq!(frame_addresses(u64::MAX), None);
        assert_eq!(restored_flags(u64::MAX) & (3 << 12 | 1 << 14 | 1 << 17), 0);
        assert_eq!(restored_flags(0), 0x202);
    }
}
