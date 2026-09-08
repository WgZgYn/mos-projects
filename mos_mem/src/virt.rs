use mos_abi::error::{KError, KResult};
use crate::phys::PhysicalFrameManager;
use crate::{PageTableManager, phys_to_virt, write_to_phys};
use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cmp::Ordering;
use core::fmt::{Debug, Formatter};
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_arch::sync::SpinIrqLock;
use mos_lib::utils::Defer;
use vfs_traits::Inode;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::mapper::TranslateError;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{
    FrameDeallocator, Mapper, OffsetPageTable, Page, PageTableFlags, PhysFrame, Size4KiB,
    Translate,
};
use x86_64::{PhysAddr, VirtAddr};
use vfs_common::IoContext;

const MEMORY_DEFAULT_HEAP_START_PAGE: Page = Page::containing_address(VirtAddr::new(0x1000_0000));
const MEMORY_DEFAULT_MMAP_START_PAGE: Page = Page::containing_address(VirtAddr::new(0x4000_0000));
const MEMORY_DEFAULT_MMAP_MAX_END_PAGE: Page =
    Page::containing_address(VirtAddr::new(0x0001_0000_0000));

#[derive(Copy, Clone, Debug)]
pub struct MemoryFlags {
    pub prot: MemoryProtFlags,
    pub map: MemoryMapFlags,
}

impl MemoryFlags {
    pub const fn new(prot: MemoryProtFlags, map: MemoryMapFlags) -> Self {
        Self { prot, map }
    }
}

/// the PageTableFlags default with PRESENT if READ
pub fn flags_prot_to_pte(value: MemoryProtFlags, user_accessible: bool) -> PageTableFlags {
    let mut flags = PageTableFlags::empty();
    if value.contains(MemoryProtFlags::READ) | value.contains(MemoryProtFlags::WRITE) {
        flags.insert(PageTableFlags::PRESENT);
    }
    if value.contains(MemoryProtFlags::WRITE) {
        flags.insert(PageTableFlags::WRITABLE);
    }
    if !value.contains(MemoryProtFlags::EXEC) {
        flags.insert(PageTableFlags::NO_EXECUTE);
    }
    if user_accessible {
        flags.insert(PageTableFlags::USER_ACCESSIBLE);
    }
    flags
}

pub struct MemorySet {
    page_table_root: PhysFrame,
    is_user_space: bool,
    pub addr_space: SpinIrqLock<MemoryAreas>,
}

pub struct MemoryAreas {
    /// 内存区域列表
    areas: BTreeMap<Page, MemoryArea>,

    mapper: OffsetPageTable<'static>,

    /// [heap_start, current_brk)
    heap: PageRange,

    mmap_base: Page,
}

impl Debug for MemoryAreas {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MemoryAreas")
            .field("areas", &self.areas)
            .field("heap", &self.heap)
            .field("mmap_base", &self.mmap_base)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct MemoryArea {
    range: PageRange,
    flags: MemoryFlags,    // 权限标志
    backing: BackingStore, // 后端存储类型
}

#[derive(Clone)]
pub struct MapFile {
    inode: Arc<dyn Inode>,
    offset: u64,
}

impl MapFile {
    pub fn new(inode: Arc<dyn Inode>, offset: u64) -> Self {
        Self { inode, offset }
    }
}

#[derive(Clone)]
pub enum BackingStore {
    Anonymous, // 匿名映射
    File(MapFile),
    Device {
        // 设备映射
        phys_addr: PhysAddr,
    },
}

impl Debug for BackingStore {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            BackingStore::Anonymous => f.write_str("Backing: Anonymous"),
            BackingStore::File(_) => f.write_str("Backing: File"),
            BackingStore::Device { .. } => f.write_str("Backing: Device"),
        }
    }
}

impl MemoryArea {
    pub fn new(range: PageRange, flags: MemoryFlags, backing: BackingStore) -> Self {
        Self {
            range,
            flags,
            backing,
        }
    }
    pub fn new_anon(range: PageRange, flags: MemoryFlags) -> Self {
        Self::new(range, flags, BackingStore::Anonymous)
    }
    pub fn is_anonymous(&self) -> bool {
        matches!(self.backing, BackingStore::Anonymous)
    }
    pub fn start(&self) -> Page {
        self.range.start
    }
    pub fn end(&self) -> Page {
        self.range.end
    }
}

impl MemoryAreas {
    pub fn new(mapper: OffsetPageTable<'static>) -> Self {
        Self {
            mapper,
            areas: BTreeMap::new(),
            heap: PageRange {
                start: MEMORY_DEFAULT_HEAP_START_PAGE,
                end: MEMORY_DEFAULT_HEAP_START_PAGE,
            },
            mmap_base: MEMORY_DEFAULT_MMAP_START_PAGE,
        }
    }

    pub fn clear(&mut self) {
        self.areas.clear();
        self.heap = PageRange {
            start: MEMORY_DEFAULT_HEAP_START_PAGE,
            end: MEMORY_DEFAULT_HEAP_START_PAGE,
        };
    }

    pub fn init_heap(&mut self, start: Page) -> KResult<()> {
        if self.find_area(start).is_some() {
            return Err(KError::Overflow);
        }
        self.heap.start = start;
        self.heap.end = self.heap.start;
        Ok(())
    }

    pub fn current_brk(&self) -> Page {
        self.heap.end
    }

    fn extend_heap(&mut self, new_brk: Page, old_end: Page) -> KResult<()> {
        let extend_range = PageRange {
            start: old_end,
            end: new_brk,
        }; // [old_end, new_brk)

        if self.check_overlap(&extend_range) {
            return Err(KError::Overflow);
        }

        let heap_key = self.heap.start;

        const DEFAULT_HEAP_FLAGS: MemoryFlags =
            MemoryFlags::new(MemoryProtFlags::READ_WRITE, MemoryMapFlags::ANONYMOUS);

        // 统一处理现有/第一次（减少if）
        let full_range = PageRange {
            start: self.heap.start,
            end: new_brk,
        };

        self.areas
            .entry(heap_key)
            .or_insert(MemoryArea::new_anon(full_range, DEFAULT_HEAP_FLAGS))
            .range
            .end = new_brk;

        self.heap.end = new_brk;
        Ok(())
    }

    // pre-check new_brk is in [old_start, old_end)
    fn shrink_heap(&mut self, new_brk: Page, _old_end: Page) -> KResult<()> {
        let heap_key = self.heap.start;
        if new_brk == self.heap.start {
            self.areas.remove(&heap_key); // 移除空Area
        } else if let Some(area) = self.areas.get_mut(&heap_key) {
            area.range.end = new_brk;
        } else {
            return Err(KError::OutOfBounds); // 防御panic
        }
        self.heap.end = new_brk;
        Ok(())
    }

    pub fn set_brk(&mut self, new_brk: Page) -> KResult<()> {
        if new_brk < self.heap.start {
            return Err(KError::InvalidArg);
        }

        match new_brk.cmp(&self.heap.end) {
            Ordering::Less => self.shrink_heap(new_brk, self.heap.end),
            Ordering::Equal => Ok(()),
            Ordering::Greater => self.extend_heap(new_brk, self.heap.end),
        }
    }

    fn find_area(&self, page: Page) -> Option<&MemoryArea> {
        // 查找 <= page 的最大键，然后检查是否包含
        self.areas.range(..=page).next_back().and_then(|(_, area)| {
            if area.range.start <= page && page < area.range.end {
                Some(area)
            } else {
                None
            }
        })
    }

    fn find_area_mut(&mut self, page: Page) -> Option<&mut MemoryArea> {
        // 查找 <= page 的最大键，然后检查是否包含
        self.areas
            .range_mut(..=page)
            .next_back()
            .and_then(|(_, area)| {
                if area.range.start <= page && page < area.range.end {
                    Some(area)
                } else {
                    None
                }
            })
    }

    fn find_free_area(&self, pages: u64) -> Option<PageRange> {
        self.find_free_area_with_start_hint(self.mmap_base, pages)
    }

    fn find_free_area_with_start_hint(&self, start: Page, pages: u64) -> Option<PageRange> {
        let mut cursor = start;

        for (_, area) in self.areas.range(start..) {
            let area_start = area.start();

            // gap = start - cursor
            if area_start > cursor {
                let gap = area_start - cursor;

                if gap >= pages {
                    return Some(PageRange {
                        start: cursor,
                        end: cursor + pages,
                    });
                }
            }

            cursor = area.end();
            if cursor >= MEMORY_DEFAULT_MMAP_MAX_END_PAGE {
                return None;
            }
        }

        // 检查最后一个 VMA 之后
        Some(PageRange {
            start: cursor,
            end: start + pages,
        })
    }

    fn mmap(
        &mut self,
        start: Option<Page>,
        pages: u64,
        flags: MemoryFlags,
        backing: BackingStore,
    ) -> KResult<PageRange> {
        let range = match start {
            Some(start) => {
                let range = PageRange {
                    start,
                    end: start + pages,
                };
                let high = start >= self.mmap_base;
                let fixed = flags.map.contains(MemoryMapFlags::FIXED);
                if (high || fixed) && !self.check_overlap(&range) {
                    range
                } else if fixed {
                    // Linux MAP_FIXED: discard whatever was mapped there
                    self.remove_range(range.start, range.end)?;
                    range
                } else if high {
                    // hint not usable, fall back to a free area
                    self.find_free_area(pages).ok_or(KError::NotMapped)?
                } else {
                    return Err(KError::NotMapped);
                }
            }
            None => self.find_free_area(pages).ok_or(KError::NotMapped)?,
        };

        self.try_add_area(MemoryArea::new(range, flags, backing))?;
        Ok(range)
    }

    /// Remove any areas overlapping [start, end), keeping the untouched
    /// head/tail pieces; PTEs of the removed pages are unmapped so the
    /// pages re-fault under their new mapping (MAP_FIXED semantics).
    fn remove_range(&mut self, start: Page, end: Page) -> KResult<()> {
        let mut removed: Vec<PageRange> = Vec::new();
        let mut affected: Vec<(Page, MemoryArea)> = Vec::new();
        let mut upper = end;
        loop {
            match self.areas.range(..upper).next_back() {
                Some((&key, area)) if area.range.end > start => {
                    affected.push((key, area.clone()));
                    upper = key;
                }
                _ => break,
            }
        }
        for (key, area) in affected {
            self.areas.remove(&key);
            let mid_start = area.range.start.max(start);
            let mid_end = area.range.end.min(end);
            removed.push(PageRange {
                start: mid_start,
                end: mid_end,
            });
            if area.range.start < start {
                self.areas.insert(
                    area.range.start,
                    MemoryArea {
                        range: PageRange {
                            start: area.range.start,
                            end: start,
                        },
                        ..area.clone()
                    },
                );
            }
            if mid_end < area.range.end {
                self.areas.insert(
                    mid_end,
                    MemoryArea {
                        range: PageRange {
                            start: mid_end,
                            end: area.range.end,
                        },
                        ..area
                    },
                );
            }
        }
        for range in removed {
            for page in (PageRange {
                start: range.start,
                end: range.end,
            }) {
                if self.mapper.translate_page(page).is_ok() {
                    unsafe {
                        let _ = self.mapper.unmap(page).map(|(_, f)| f.flush());
                    }
                }
            }
        }
        Ok(())
    }

    fn munmap(&mut self, start: Page, pages: u64) -> KResult<()> {
        let area = self.areas.get(&start).ok_or(KError::NotMapped)?;
        if area.range.len() == pages {
            self.areas.remove(&start);
            Ok(())
        } else {
            Err(KError::InvalidArg)
        }
    }

    /// mprotect(2): change the protection of every page in [start, end).
    /// Every page must already be covered by an area (ENOMEM otherwise);
    /// partially covered areas are split so the invariant
    /// "areas never overlap" is kept.
    fn protect(
        &mut self,
        start: Page,
        end: Page,
        prot: MemoryProtFlags,
        user_accessible: bool,
    ) -> KResult<()> {
        // validate coverage: walk the areas, jumping from one area end to
        // the next, until the whole range is covered
        let mut cursor = start;
        while cursor < end {
            match self.find_area(cursor) {
                Some(area) => cursor = area.range.end,
                None => return Err(KError::NotMapped), // ENOMEM
            }
        }

        // collect the areas intersecting [start, end), from high key to low
        let mut affected: Vec<(Page, MemoryArea)> = Vec::new();
        let mut upper = end;
        loop {
            match self.areas.range(..upper).next_back() {
                Some((&key, area)) if area.range.end > start => {
                    affected.push((key, area.clone()));
                    upper = key;
                }
                _ => break,
            }
        }

        // replace each affected area with its unchanged head/tail plus a
        // middle piece carrying the new protection
        for (key, area) in affected {
            self.areas.remove(&key);
            let (area_start, area_end) = (area.range.start, area.range.end);
            if area_start < start {
                self.areas.insert(
                    area_start,
                    MemoryArea {
                        range: PageRange {
                            start: area_start,
                            end: start,
                        },
                        ..area.clone()
                    },
                );
            }
            let mid_start = area_start.max(start);
            let mid_end = area_end.min(end);
            self.areas.insert(
                mid_start,
                MemoryArea {
                    range: PageRange {
                        start: mid_start,
                        end: mid_end,
                    },
                    flags: MemoryFlags::new(prot, area.flags.map),
                    backing: area.backing.clone(),
                },
            );
            if mid_end < area_end {
                self.areas.insert(
                    mid_end,
                    MemoryArea {
                        range: PageRange {
                            start: mid_end,
                            end: area_end,
                        },
                        ..area.clone()
                    },
                );
            }
        }

        // update page tables for pages already mapped; unmapped pages pick
        // the new flags up when the fault handler maps them
        let pte_flags = flags_prot_to_pte(prot, user_accessible);
        for page in (PageRange { start, end }) {
            if self.mapper.translate_page(page).is_ok() {
                unsafe {
                    let _ = self.mapper.update_flags(page, pte_flags);
                }
            }
        }
        Ok(())
    }

    fn check_overlap(&self, new_range: &PageRange) -> bool {
        if let Some((_, area)) = self.areas.range(..new_range.end).next_back() {
            new_range.start < area.end()
        } else {
            false
        }
    }

    /// only record the virtual memory field, not alloc the frame
    /// 热切映射物理帧到当前页表。
    /// 用于需要直接控制物理地址的场景 (如用户态 framebuffer)。
    pub fn map_page_eagerly(
        &mut self,
        page: Page<Size4KiB>,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
    ) -> Result<(), MapToError<Size4KiB>> {
        unsafe {
            self.mapper
                .map_to(page, frame, flags, &mut PhysicalFrameManager)?
                .flush();
        }
        Ok(())
    }

    pub fn try_add_area(&mut self, area: MemoryArea) -> KResult<()> {
        // check overlap
        if self.check_overlap(&area.range) {
            return Err(KError::AlreadyMapped);
        }
        self.areas.insert(area.range.start, area);
        Ok(())
    }

    pub fn get_mapper(&mut self) -> &mut OffsetPageTable<'static> {
        &mut self.mapper
    }
}

impl MemorySet {
    pub fn new(
        (page_table_root, mapper): (PhysFrame, OffsetPageTable<'static>),
        is_user_space: bool,
    ) -> Self {
        Self {
            page_table_root,
            is_user_space,
            addr_space: SpinIrqLock::new(MemoryAreas::new(mapper)),
        }
    }

    pub fn clear(&self) {
        self.addr_space.access(|addr_space| addr_space.clear());
        let root = self.page_table_frame();
        PageTableManager::free_user_page_table(root);
    }

    pub fn page_table_frame(&self) -> PhysFrame {
        self.page_table_root
    }

    /// this will eagerly alloc frame and load data
    pub fn load_data(
        &self,
        mut vaddr: VirtAddr,
        memsz: u64,
        mut data: &[u8],
        prot_flags: MemoryProtFlags,
    ) -> KResult<()> {
        if memsz < data.len() as _ {
            return Err(KError::InvalidArg);
        }

        // add virtual memory area
        let start = Page::containing_address(vaddr);
        let end = Page::containing_address((vaddr + memsz).align_up(4096_u64)); // excluded
        let page_range = PageRange { start, end };
        let mem_flags = MemoryFlags::new(prot_flags, MemoryMapFlags::PRIVATE);
        let mem_area = MemoryArea::new_anon(page_range, mem_flags);

        self.addr_space
            .access(|addr_space| addr_space.try_add_area(mem_area))?;

        // map frame eagerly
        for page in page_range {
            let ret = self
                .addr_space
                .access(|addr_space| addr_space.mapper.translate_page(page));
            if let Err(TranslateError::PageNotMapped) = ret {
                let frame = PhysicalFrameManager
                    .global_allocate_frame()
                    .ok_or(KError::FrameAllocFailed)?;
                let flags = flags_prot_to_pte(prot_flags, self.is_user_space);
                unsafe {
                    self.addr_space
                        .access(|addr_space| {
                            addr_space
                                .mapper
                                .map_to(page, frame, flags, &mut PhysicalFrameManager)
                                .map_err(|_| "mapping frame error")
                        })?
                        .flush();
                }
            }
        }

        // load data
        let mut remains = data.len();
        while remains > 0 {
            // write from kernel space VirtAddr to user space VirtAddr
            let paddr = self.addr_space.access(|addr_space| {
                addr_space
                    .mapper
                    .translate_addr(vaddr)
                    .ok_or(KError::NotMapped)
            })?;

            let writeable = (remains as u64).min(4096 - vaddr.as_u64() % 4096);
            // the kernel space is not overlapped with user space, so it is safe.
            unsafe {
                write_to_phys(paddr, &data[..writeable as usize]);
            }
            remains -= writeable as usize;
            vaddr += writeable;
            data = &data[writeable as usize..];
        }
        Ok(())
    }

    /// # Safety
    /// this function will change the page table,
    /// make sure the code and the data is in the page table
    pub unsafe fn activate(&self) {
        unsafe { x86_64::registers::control::Cr3::write(self.page_table_root, Cr3Flags::empty()) }
        x86_64::instructions::tlb::flush_all()
    }
    pub fn is_userspace(&self) -> bool {
        self.is_user_space
    }
    pub fn share_same_page_table(&self, rhs: &Self) -> bool {
        self.page_table_root == rhs.page_table_root
    }

    pub fn share_with(&self, new: &Self) -> KResult<()> {
        // clone the vma
        self.addr_space.access(|old_mm| {
            new.addr_space.access(|new_mm| {
                new_mm.areas = old_mm.areas.clone();
                new_mm.heap = old_mm.heap.clone();
            })
        });

        // walk the page table
        PageTableManager::user_page_table_walk(self.page_table_root, |page, frame, flags| {
            // if writable, make it cow, else make it directly share with ref_inc
            if flags.contains(PageTableFlags::WRITABLE) {
                let mut cow_flags = flags;
                cow_flags.remove(PageTableFlags::WRITABLE);
                cow_flags.insert(PageTableFlags::BIT_9);

                unsafe {
                    self.addr_space
                        .access(|old_mm| {
                            old_mm
                                .mapper
                                .update_flags(page, cow_flags)
                                .map_err(|_| "mapping flags error")
                        })?
                        .flush();

                    new.addr_space
                        .access(|new_mm| {
                            new_mm
                                .mapper
                                .map_to(page, frame, cow_flags, &mut PhysicalFrameManager)
                                .map_err(|_| "mapping frame error")
                        })?
                        .flush();
                }
            } else {
                unsafe {
                    new.addr_space
                        .access(|new_mm| {
                            new_mm
                                .mapper
                                .map_to(page, frame, flags, &mut PhysicalFrameManager)
                                .map_err(|_| "mapping frame error")
                        })?
                        .flush();
                }
            }
            // if mapping success, inc the frame refcount
            PhysicalFrameManager.ref_inc(frame);
            Ok(())
        })?;

        x86_64::instructions::tlb::flush_all();

        Ok(())
    }

    pub fn try_handle_frame_alloc(&self, addr: VirtAddr) -> KResult<()> {
        let page = Page::containing_address(addr);

        let Some(area) = self
            .addr_space
            .access(|addr_space| addr_space.find_area(page).cloned())
        else {
            return Err(KError::NotMapped);
        };

        // allocate physical frame
        let frame = PhysicalFrameManager
            .global_allocate_frame()
            .ok_or(KError::FrameAllocFailed)?;

        let mut defer = Defer::new(|| unsafe { PhysicalFrameManager.deallocate_frame(frame) });

        // update the PTE flags
        unsafe {
            let flags = flags_prot_to_pte(area.flags.prot, self.is_user_space);
            let page_offset = page - area.start();
            let backing = area.backing.clone();
            let mapping = self.addr_space.access(|addr_space| {
                addr_space
                    .mapper
                    .map_to(page, frame, flags, &mut PhysicalFrameManager)
                    .map_err(|_| "map frame error")
            })?;

            match backing {
                BackingStore::Anonymous => { /* the frame is already zeroed */ }
                BackingStore::File(f) => {
                    let virt_ptr = phys_to_virt(frame.start_address()).as_mut_ptr::<u8>();
                    let buf = core::slice::from_raw_parts_mut(virt_ptr, 4096);
                    let mut offset = page_offset * 4096 + f.offset;
                    let mut total = 0;
                    loop {
                        let n = f
                            .inode
                            .as_file_ops()
                            .expect("should mmap at regular file")
                            .read(&IoContext::default(), offset as usize, &mut buf[total..])
                            .map_err(|e| e.as_str())?;
                        offset += n as u64;
                        total += n;
                        if n == 0 {
                            break;
                        }
                    }
                    if total < 4096 {
                        // info!("read partly file data, left fill zero");
                        buf[total..].fill(0);
                    }
                }
                BackingStore::Device { .. } => todo!(),
            }

            mapping.flush();
        }

        defer.cancel();
        Ok(())
    }

    pub fn mmap(
        &self,
        start: Option<Page>,
        pages: u64,
        flags: MemoryFlags,
        backing: BackingStore,
    ) -> KResult<PageRange> {
        self.addr_space
            .access(|mm| mm.mmap(start, pages, flags, backing))
    }

    pub fn munmap(&self, start: Page, pages: u64) -> KResult<()> {
        self.addr_space.access(|mm| mm.munmap(start, pages))
    }

    /// mprotect(2)
    pub fn protect(&self, start: Page, pages: u64, prot: MemoryProtFlags) -> KResult<()> {
        let end = start + pages;
        self.addr_space
            .access(|mm| mm.protect(start, end, prot, self.is_user_space))?;
        // changed PTEs may be cached in any core's TLB
        x86_64::instructions::tlb::flush_all();
        Ok(())
    }

    pub fn find_free_area(&self, pages: u64) -> Option<PageRange> {
        self.addr_space.access(|mm| mm.find_free_area(pages))
    }

    pub fn is_free_area(&self, range: PageRange) -> bool {
        !self.addr_space.access(|mm| mm.check_overlap(&range))
    }
}
