use core::mem::offset_of;
use x86_64::registers::rflags::RFlags;
use x86_64::structures::gdt::SegmentSelector;
use x86_64::structures::idt::InterruptStackFrameValue;
use x86_64::VirtAddr;

/// Caller saved registers
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct ScratchRegisters {
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rax: u64,
}

/// Callee saved registers
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct PreservedRegisters {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbp: u64,
    pub rbx: u64,
}

/// Hardware pushed
#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct IretRegisters {
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// x87/SSE context image used by `fxsave64`/`fxrstor64`.
#[repr(C, align(16))]
#[derive(Debug, Clone)]
pub struct FpuState {
    bytes: [u8; 512],
}

impl FpuState {
    pub const fn new() -> Self {
        Self { bytes: [0; 512] }
    }

    pub fn bytes(&self) -> &[u8; 512] { &self.bytes }
    pub fn bytes_mut(&mut self) -> &mut [u8; 512] { &mut self.bytes }
}

impl Default for FpuState {
    fn default() -> Self {
        Self::new()
    }
}

/// Used for kernel context switch,
/// the rsp is the kernel stack of the thread which stored the `InterruptStack`
/// the rip is the kernel context ip which will load to pop registers and iret to finally realize context switch.
#[repr(C)]
#[repr(align(16))]
#[derive(Debug, Clone, Default)]
pub struct ThreadContext {
    pub preserved: PreservedRegisters,
    pub rsp: u64,
    pub rflags: u64,
    pub fpu: FpuState,
    /// User FS base (x86-64 TLS). Restored from IA32_FS_BASE on context switch.
    /// Written by arch_prctl(ARCH_SET_FS), restored in __switch_to before flags.
    pub fs_base: u64,
}

const _: () = assert!(align_of::<FpuState>() == 16);
const _: () = assert!(size_of::<FpuState>() == 512);
const _: () = assert!(offset_of!(ThreadContext, fpu) == 0x40);
const _: () = assert!(offset_of!(ThreadContext, rflags) == 0x38);
const _: () = assert!(offset_of!(ThreadContext, fs_base) == 0x240);

impl From<IretRegisters> for InterruptStackFrameValue {
    fn from(value: IretRegisters) -> Self {
        InterruptStackFrameValue::new(
            VirtAddr::new(value.rip),
            SegmentSelector(value.cs as _),
            RFlags::from_bits_truncate(value.rflags),
            VirtAddr::new(value.rsp as _),
            SegmentSelector(value.ss as _),
        )
    }
}

#[repr(C)]
#[derive(Debug, Clone, Default)]
pub struct InterruptStack {
    pub preserved: PreservedRegisters, // 6
    pub scratch: ScratchRegisters,     // 9
    pub iret: IretRegisters,           // 5
}

// A full IRET frame separates RIP/RFLAGS from RCX/R11. sigreturn must
// restore all four independently; SYSRET cannot express that state.
pub type SyscallContext = InterruptStack;
const _: () = assert!(offset_of!(InterruptStack, iret) == 120);
const _: () = assert!(size_of::<InterruptStack>() == 160);

#[derive(Debug, Clone)]
pub struct CpuContext {
    pub cs: SegmentSelector,
    pub ss: SegmentSelector,
    pub rflags: RFlags,
}
