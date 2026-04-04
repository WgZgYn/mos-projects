#![feature(allocator_api)]
#![no_std]
extern crate alloc;

mod config;
mod heap;
mod phys;
mod ptmanager;
mod regions;
pub mod slab;
pub mod virt;

use crate::heap::init_heap;
use bootloader_api::info::MemoryRegions;
use mos_lib::KernelBootGlobal;
pub use phys::PhysicalFrameManager;
pub use ptmanager::PageTableManager;
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::{
    PhysAddr, VirtAddr,
    structures::paging::{OffsetPageTable, PageTable},
};

/// because we init the global data first in the boostrap context,
/// it's no problem to read it later.
pub static PHYSICAL_MEMORY_OFFSET: KernelBootGlobal<u64> = KernelBootGlobal::uninit();
pub static KERNEL_ADDR: KernelBootGlobal<u64> = KernelBootGlobal::uninit();
pub static KERNEL_LEN: KernelBootGlobal<u64> = KernelBootGlobal::uninit();
pub static KERNEL_IMAGE_OFFSET: KernelBootGlobal<u64> = KernelBootGlobal::uninit();

pub fn init(
    physical_memory_offset: u64,
    memory_regions: &mut MemoryRegions,
    kernel_addr: u64,
    kernel_len: u64,
    kernel_image_offset: u64,
) -> OffsetPageTable<'static> {
    // init the global data
    unsafe {
        PHYSICAL_MEMORY_OFFSET.init(|| physical_memory_offset);
        KERNEL_ADDR.init(|| kernel_addr);
        KERNEL_LEN.init(|| kernel_len);
        KERNEL_IMAGE_OFFSET.init(|| kernel_image_offset);
    }

    // init mapper
    let mut mapper = unsafe { init_mapper(VirtAddr::new(physical_memory_offset)) };

    // init usable memory regions and init physical frame manager
    regions::init(memory_regions);
    regions::access(|regions| PhysicalFrameManager::init(regions));

    // init kernel heap space
    init_heap(&mut mapper, &mut PhysicalFrameManager).expect("Failed to initialize kernel heap");

    // for other use
    mapper
}

/// # Safety
/// must call and only once
unsafe fn init_mapper(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    /// 返回一个对活动的4级表的可变引用。
    ///
    /// 这个函数是不安全的，因为调用者必须保证完整的物理内存在传递的
    /// `physical_memory_offset`处被映射到虚拟内存。另外，这个函数
    /// 必须只被调用一次，以避免别名"&mut "引用（这是未定义的行为）。
    unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
        use x86_64::registers::control::Cr3;

        let (level_4_table_frame, _) = Cr3::read();

        let phys = level_4_table_frame.start_address();
        let virt = physical_memory_offset + phys.as_u64();
        let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

        unsafe { &mut *page_table_ptr }
    }
    unsafe {
        let level_4_table = active_level_4_table(physical_memory_offset);
        OffsetPageTable::new(level_4_table, physical_memory_offset)
    }
}

/// # Safety
/// the data addr points must not overlap with data
pub unsafe fn write_to_phys(addr: PhysAddr, data: &[u8]) {
    unsafe {
        let vaddr = phys_to_virt(addr);
        vaddr
            .as_mut_ptr::<u8>()
            .copy_from_nonoverlapping(data.as_ptr(), data.len());
    }
}

/// # Safety
/// the data addr points must not overlap with data
pub unsafe fn write_zero_byte_to_phys(addr: PhysAddr, len: usize) {
    unsafe {
        let vaddr = phys_to_virt(addr);
        vaddr.as_mut_ptr::<u8>().write_bytes(0, len)
    }
}

pub fn zero_frame(f: PhysFrame<Size4KiB>) {
    unsafe {
        write_zero_byte_to_phys(f.start_address(), 4096);
    }
}

/// translate phys addr to virt addr by offset mapping, which need init the page-table
pub fn phys_to_virt(phys: PhysAddr) -> VirtAddr {
    unsafe { VirtAddr::new(phys.as_u64() + PHYSICAL_MEMORY_OFFSET.get()) }
}

pub fn phys_to_virt_page(phys: PhysFrame) -> Page {
    unsafe {
        Page::containing_address(VirtAddr::new(
            phys.start_address().as_u64() + PHYSICAL_MEMORY_OFFSET.get(),
        ))
    }
}

pub fn phys_page_copy(src: Page, dst: PhysFrame) {
    unsafe {
        write_to_phys(
            dst.start_address(),
            core::slice::from_raw_parts(src.start_address().as_ptr::<u8>(), 4096),
        )
    }
}

pub fn copy_from_user() {}

pub fn copy_to_user() {}
