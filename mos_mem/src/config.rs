pub const MEMORY_REGION_RECORD: usize = 128;
pub const PHYS_ALLOCATE_START: u64 = PAGE_SIZE_BYTE as u64 * 1024; // 1 MiB
pub const PHYS_RAMFS_START: u64 = 0x5000_0000;
pub const PHYS_RAMFS_SIZE: u64 = 4 * 1024 * 1024; // 4MB
pub const PHYS_RAMFS_END: u64 = PHYS_RAMFS_START + PHYS_RAMFS_SIZE;

/// ordered, preserve memory fields
pub const PRESERVE_MEMORY_REGIONS: [(u64, u64); 2] =
    [(0, PHYS_ALLOCATE_START), (PHYS_RAMFS_START, PHYS_RAMFS_END)];

pub const PAGE_SIZE_BYTE: usize = 1024 * 4; // 4 KiB
pub const GLOBAL_HEAP_START: usize = 0xFFFF_FFFF_0000_0000;
pub const GLOBAL_HEAP_SIZE: usize = 16 * 1024 * 1024; // 16 MiB
pub const GLOBAL_HEAP_END: usize = GLOBAL_HEAP_START + GLOBAL_HEAP_SIZE;
const _: () = assert!(GLOBAL_HEAP_START.is_multiple_of(PAGE_SIZE_BYTE));

pub const BUDDY_METADATA_HEAP_SIZE: u64 = 1024 * 1024 * 16; // 16 MiB
