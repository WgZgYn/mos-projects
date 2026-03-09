use bitflags::bitflags;

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
        const PRIVATE = 1 << 0;
        const SHARED = 1 << 1;

        const FIXED = 1 << 4;
        const ANONYMOUS = 1 << 5;
        const POPULATE = 1 << 15;
    }

    #[derive(Copy, Clone)]
    pub struct MemoryAccess: u32 {
        const KERNEL_ONLY = 1 << 0;
        const USER_ACCESSIBLE = 1 << 1;
    }
}