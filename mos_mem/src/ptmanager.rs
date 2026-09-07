use mos_abi::error::{KError, KResult};
use crate::phys::PhysicalFrameManager;
use crate::{PHYSICAL_MEMORY_OFFSET, zero_frame};
use lazy_static::lazy_static;
use mos_arch::page_table_base_frame;
use x86_64::VirtAddr;
use x86_64::structures::paging::{
    OffsetPageTable, Page, PageTable, PageTableFlags, PageTableIndex, PhysFrame,
};

lazy_static! {
    pub static ref PAGE_TABLE_MANAGER: PageTableManager = PageTableManager::new(
        unsafe { *PHYSICAL_MEMORY_OFFSET.get() },
        page_table_base_frame()
    );
}

pub struct PageTableManager {
    phys_memory_offset: u64,
    kernel_pbr: PhysFrame,
}

impl PageTableManager {
    const fn new(pmo: u64, kpbr: PhysFrame) -> Self {
        Self {
            phys_memory_offset: pmo,
            kernel_pbr: kpbr,
        }
    }
    pub fn kernel_bpr() -> PhysFrame {
        PAGE_TABLE_MANAGER.kernel_pbr
    }

    pub fn copy_root_page_table() -> PageTable {
        unsafe {
            let page: *mut PageTable = Self::page_table_instance(PAGE_TABLE_MANAGER.kernel_pbr);
            (*page).clone()
        }
    }

    pub fn current_page_table_root_frame() -> PhysFrame {
        x86_64::registers::control::Cr3::read().0
    }

    fn page_table_instance(bpr: PhysFrame) -> *mut PageTable {
        (bpr.start_address().as_u64() + PAGE_TABLE_MANAGER.phys_memory_offset) as *mut PageTable
    }

    /// # Safety
    /// user_bpr must be legal
    unsafe fn init_page_table_with_kernel_high_half(user_bpr: PhysFrame) {
        unsafe {
            let user = Self::page_table_instance(user_bpr);
            let kernel = Self::page_table_instance(Self::kernel_bpr());
            core::ptr::copy_nonoverlapping(kernel, user, 1);
            let user = &mut *user;
            for i in 0..256 {
                user[i].set_unused();
            }
        }
    }

    /// # Safety
    /// pbr must be legal
    unsafe fn offset_mapper_of(pbr: PhysFrame) -> OffsetPageTable<'static> {
        unsafe {
            OffsetPageTable::new(
                &mut *Self::page_table_instance(pbr),
                VirtAddr::new(PAGE_TABLE_MANAGER.phys_memory_offset),
            )
        }
    }

    pub fn alloc_page_table() -> KResult<(PhysFrame, OffsetPageTable<'static>)> {
        let frame = PhysicalFrameManager
            .global_allocate_frame()
            .ok_or(KError::FrameAllocFailed)?;

        unsafe {
            zero_frame(frame);
            Self::init_page_table_with_kernel_high_half(frame);
            Ok((frame, Self::offset_mapper_of(frame)))
        }
    }

    pub fn kernel_page_table() -> (PhysFrame, OffsetPageTable<'static>) {
        let frame = PAGE_TABLE_MANAGER.kernel_pbr;
        unsafe { (frame, Self::offset_mapper_of(frame)) }
    }

    /// # Safety
    /// get memory mapper
    pub unsafe fn current_mapper() -> OffsetPageTable<'static> {
        unsafe { Self::offset_mapper_of(Self::current_page_table_root_frame()) }
    }

    pub fn user_page_table_walk(
        root: PhysFrame,
        mut f: impl FnMut(Page, PhysFrame, PageTableFlags) -> Result<(), &'static str>,
    ) -> Result<(), &'static str> {
        let table = unsafe { &*PageTableManager::page_table_instance(root) };
        for (i, entry) in table.iter().enumerate().take(256) {
            if !entry.flags().contains(PageTableFlags::PRESENT) {
                continue;
            }
            let frame = entry.frame().unwrap();
            let table = unsafe { &*PageTableManager::page_table_instance(frame) };
            for (j, entry) in table.iter().enumerate() {
                if !entry.flags().contains(PageTableFlags::PRESENT) {
                    continue;
                }
                let frame = entry.frame().unwrap();
                let table = unsafe { &*PageTableManager::page_table_instance(frame) };
                for (k, entry) in table.iter().enumerate() {
                    if !entry.flags().contains(PageTableFlags::PRESENT) {
                        continue;
                    }
                    let frame = entry.frame().unwrap();
                    let table = unsafe { &*PageTableManager::page_table_instance(frame) };
                    for (m, entry) in table.iter().enumerate() {
                        if !entry.flags().contains(PageTableFlags::PRESENT) {
                            continue;
                        }
                        let frame = entry.frame().unwrap();
                        let flags = entry.flags();

                        let page = Page::from_page_table_indices(
                            PageTableIndex::new(i as _),
                            PageTableIndex::new(j as _),
                            PageTableIndex::new(k as _),
                            PageTableIndex::new(m as _),
                        );

                        f(page, frame, flags)?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn free_cr3(frame: PhysFrame) -> u64 {
        Self::free_user_page_table(frame)
            + if PhysicalFrameManager.ref_dec(frame) {
                1
            } else {
                0
            }
    }

    pub fn free_user_page_table(frame: PhysFrame) -> u64 {
        let mut n = 0;
        let table = unsafe { &mut *PageTableManager::page_table_instance(frame) };
        for entry in table.iter_mut().take(256) {
            if !entry.flags().contains(PageTableFlags::PRESENT) {
                continue;
            }
            let frame = entry.frame().unwrap();
            let table = unsafe { &mut *PageTableManager::page_table_instance(frame) };
            for entry in table.iter_mut() {
                if !entry.flags().contains(PageTableFlags::PRESENT) {
                    continue;
                }
                let frame = entry.frame().unwrap();
                let table = unsafe { &mut *PageTableManager::page_table_instance(frame) };
                for entry in table.iter_mut() {
                    if !entry.flags().contains(PageTableFlags::PRESENT) {
                        continue;
                    }
                    let frame = entry.frame().unwrap();
                    let table = unsafe { &mut *PageTableManager::page_table_instance(frame) };
                    for entry in table.iter_mut() {
                        if !entry.flags().contains(PageTableFlags::PRESENT) {
                            continue;
                        }
                        let frame = entry.frame().unwrap();
                        entry.set_unused();
                        if PhysicalFrameManager.ref_dec(frame) {
                            n += 1
                        }
                    }
                    entry.set_unused();
                    if PhysicalFrameManager.ref_dec(frame) {
                        n += 1
                    }
                }
                entry.set_unused();
                if PhysicalFrameManager.ref_dec(frame) {
                    n += 1
                }
            }
            entry.set_unused();
            if PhysicalFrameManager.ref_dec(frame) {
                n += 1
            }
        }
        n
    }
}
