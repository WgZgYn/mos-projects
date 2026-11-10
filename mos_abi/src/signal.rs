//! Linux x86_64 *kernel* signal ABI; libc's public sigset_t can be larger.
//! Layout: arch/x86/include/uapi/asm/signal.h in the Linux source tree.
use crate::error::{KError, KResult};

pub const SIGNAL_COUNT: usize = 64;
pub const SIGSET_BYTES: usize = 8;
pub const SIGKILL: i32 = 9;
pub const SIGINT: i32 = 2;
pub const SIGUSR1: i32 = 10;
pub const SIGUSR2: i32 = 12;
pub const SIGPIPE: i32 = 13;
pub const SIGCHLD: i32 = 17;
pub const SIGSTOP: i32 = 19;
pub const SIG_DFL: u64 = 0;
pub const SIG_IGN: u64 = 1;
pub const SIG_BLOCK: i32 = 0;
pub const SIG_UNBLOCK: i32 = 1;
pub const SIG_SETMASK: i32 = 2;
pub const SA_NOCLDWAIT: u64 = 2;
pub const SA_RESTORER: u64 = 0x0400_0000;
pub const SA_SIGINFO: u64 = 4;
pub const SA_RESTART: u64 = 0x1000_0000;
pub const SA_NODEFER: u64 = 0x4000_0000;
pub const SA_RESETHAND: u64 = 0x8000_0000;
pub const SIGSEGV: i32 = 11;

pub const fn signal_bit(signal: i32) -> u64 {
    if signal >= 1 && signal <= SIGNAL_COUNT as i32 { 1u64 << (signal - 1) } else { 0 }
}

/// SIGKILL and SIGSTOP cannot be blocked, even by an all-ones user mask.
pub const fn blockable(mask: u64) -> u64 {
    mask & !(signal_bit(SIGKILL) | signal_bit(SIGSTOP))
}

pub fn change_mask(old: u64, set: u64, how: i32) -> KResult<u64> {
    Ok(blockable(match how {
        SIG_BLOCK => old | set,
        SIG_UNBLOCK => old & !set,
        SIG_SETMASK => set,
        _ => return Err(KError::InvalidArg),
    }))
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KernelSigAction {
    pub handler: u64,
    pub flags: u64,
    pub restorer: u64,
    pub mask: u64,
}

impl KernelSigAction {
    // Explicit byte conversion avoids alignment assumptions about user pointers
    // and prevents exposing padding or constructing references to user memory.
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        let mut words = [0u64; 4];
        for (word, chunk) in words.iter_mut().zip(bytes.chunks_exact(8)) {
            *word = u64::from_ne_bytes(chunk.try_into().unwrap());
        }
        Self { handler: words[0], flags: words[1], restorer: words[2], mask: words[3] }
    }

    pub fn to_bytes(self) -> [u8; 32] {
        let mut bytes = [0; 32];
        for (chunk, word) in bytes.chunks_exact_mut(8).zip([self.handler, self.flags, self.restorer, self.mask]) {
            chunk.copy_from_slice(&word.to_ne_bytes());
        }
        bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_action_layout_and_roundtrip() {
        assert_eq!(size_of::<KernelSigAction>(), 32);
        assert_eq!(align_of::<KernelSigAction>(), 8);
        assert_eq!(core::mem::offset_of!(KernelSigAction, flags), 8);
        assert_eq!(core::mem::offset_of!(KernelSigAction, restorer), 16);
        assert_eq!(core::mem::offset_of!(KernelSigAction, mask), 24);
        let action = KernelSigAction { handler: 1, flags: SA_RESTORER, restorer: 0x1234, mask: 0x5678 };
        assert_eq!(KernelSigAction::from_bytes(action.to_bytes()), action);
    }

    #[test]
    fn masks_cover_bit_64_and_cannot_block_kill_or_stop() {
        assert_eq!(signal_bit(64), 1 << 63);
        assert_eq!(signal_bit(0), 0);
        assert_eq!(signal_bit(65), 0);
        let mask = change_mask(0, u64::MAX, SIG_BLOCK).unwrap();
        assert_eq!(mask & signal_bit(SIGKILL), 0);
        assert_eq!(mask & signal_bit(SIGSTOP), 0);
        assert_ne!(mask & signal_bit(64), 0);
        assert_eq!(change_mask(mask, u64::MAX, SIG_UNBLOCK), Ok(0));
        assert_eq!(change_mask(mask, signal_bit(SIGUSR1), SIG_SETMASK), Ok(signal_bit(SIGUSR1)));
        assert_eq!(change_mask(0, 0, 3), Err(KError::InvalidArg));
    }
}
