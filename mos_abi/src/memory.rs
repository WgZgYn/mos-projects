use bitflags::bitflags;

// Linux PROT_* values for mprotect(2)/mmap(2) — same bits as MemoryProtFlags
pub const PROT_READ: u32 = 0x1;
pub const PROT_WRITE: u32 = 0x2;
pub const PROT_EXEC: u32 = 0x4;
pub const PROT_NONE: u32 = 0x0;

bitflags! {
    #[derive(Copy, Clone, Debug)]
    pub struct MemoryProtFlags: u32 {
        const READ = 1 << 0;
        const WRITE = 1 << 1;
        const EXEC = 1 << 2;

        const READ_WRITE = MemoryProtFlags::READ.bits() | MemoryProtFlags::WRITE.bits();
    }

    #[derive(Copy, Clone, Debug)]
    pub struct MemoryMapFlags: u32 {
        // bit values follow Linux MAP_* so musl/glibc pass them directly
        const SHARED = 1 << 0;   // MAP_SHARED
        const PRIVATE = 1 << 1;  // MAP_PRIVATE

        const FIXED = 1 << 4;       // MAP_FIXED
        const ANONYMOUS = 1 << 5;   // MAP_ANONYMOUS
        const POPULATE = 1 << 15;   // MAP_POPULATE
    }

    #[derive(Copy, Clone)]
    pub struct MemoryAccess: u32 {
        const KERNEL_ONLY = 1 << 0;
        const USER_ACCESSIBLE = 1 << 1;
    }
}
