use mos_abi::error::{KError, KResult};
use crate::config::{MEMORY_REGION_RECORD, PRESERVE_MEMORY_REGIONS};
use crate::{KERNEL_ADDR, KERNEL_LEN};
use bootloader_api::info::{MemoryRegionKind, MemoryRegions};
use core::fmt::{Debug, Formatter};
use log::info;
use mos_arch::sync::IrqLocked;
use mos_lib::align::{align_down, align_up};

static GLOBAL_USABLE_MEMORY: IrqLocked<UsableMemoryRegions> =
    unsafe { IrqLocked::new(UsableMemoryRegions::uninit()) };

/// [start, end), aligned to 4Kib
#[derive(Copy, Clone, Debug)]
pub struct UsableMemoryRegion {
    pub start: u64,
    pub end: u64,
}

/// This struct will record the memory from the boot info
/// it will merge the free regions and align it to PageSize of 4KiB
pub struct UsableMemoryRegions {
    regions: [UsableMemoryRegion; MEMORY_REGION_RECORD],
    size: usize,
}

pub fn init(data: &MemoryRegions) {
    GLOBAL_USABLE_MEMORY.access(|mem_regions| {
        mem_regions.init(data);
        if let Err(e) = mem_regions.check() {
            panic!("usable memory regions initialize failed: {}", e);
        }
    })
}

pub fn access<R, F: FnOnce(&mut UsableMemoryRegions) -> R>(f: F) -> R {
    GLOBAL_USABLE_MEMORY.access(f)
}

impl UsableMemoryRegion {
    const fn from(start: u64, end: u64) -> Option<UsableMemoryRegion> {
        let start = align_up(start, 4096);
        let end = align_down(end, 4096);
        if start >= end {
            return None;
        }
        Some(UsableMemoryRegion { start, end })
    }
    pub const fn from_exactly(start: u64, end: u64) -> Option<UsableMemoryRegion> {
        if start >= end {
            return None;
        };
        if !start.is_multiple_of(4096) || !end.is_multiple_of(4096) {
            return None;
        }
        Some(UsableMemoryRegion { start, end })
    }
    pub const fn frames_count(&self) -> usize {
        (self.end - self.start) as usize / 4096
    }
}

impl UsableMemoryRegions {
    const fn uninit() -> Self {
        UsableMemoryRegions {
            regions: [UsableMemoryRegion { start: 0, end: 0 }; MEMORY_REGION_RECORD],
            size: 0,
        }
    }
    pub fn usable_regions(&self) -> &[UsableMemoryRegion] {
        &self.regions[..self.size]
    }
    fn is_address_overlap(&self, start: u64, end: u64) -> Option<usize> {
        if end <= start {
            return None;
        }
        for (i, region) in self.usable_regions().iter().enumerate() {
            if start < region.end && end > region.start {
                return Some(i);
            }
        }
        None
    }

    pub fn remove(&mut self, i: usize) {
        if i >= self.size {
            return;
        }
        for i in i..self.size - 1 {
            self.regions[i] = self.regions[i + 1];
        }
        self.regions[self.size - 1] = UsableMemoryRegion { start: 0, end: 0 };
        self.size -= 1;
    }

    /// the region must in the range of the original
    pub fn shrink(&mut self, index: usize, region: UsableMemoryRegion) -> bool {
        if index >= self.size {
            return false;
        }
        let old = self.regions[index];
        if old.start > region.start || old.end < region.end {
            return false;
        }
        self.regions[index] = region;
        true
    }

    /// It will ignore if regions count is more than `MEMORY_REGION_RECORD`
    fn push(&mut self, region: UsableMemoryRegion) {
        if self.size >= MEMORY_REGION_RECORD {
            return;
        }
        self.regions[self.size] = region;
        self.size += 1;
    }

    fn push_sub_preserved(&mut self, region: UsableMemoryRegion) {
        enum SubtractResult {
            NoSubtract,
            SubtractAll,
            SubtractCenter,
            SubtractLeft,
            SubtractRight,
        }
        fn region_subtract(start: u64, end: u64, p_start: u64, p_end: u64) -> SubtractResult {
            assert!(p_start < p_end);
            assert!(start < end);
            // 在范围外
            if end <= p_start || start >= p_end {
                return SubtractResult::NoSubtract;
            }
            // 在范围内
            if p_start <= start && p_end >= end {
                return SubtractResult::SubtractAll;
            }
            // 截取中间
            if p_start > start && p_end < end {
                return SubtractResult::SubtractCenter;
            }
            // 左裁剪
            if p_start <= start && p_end < end {
                return SubtractResult::SubtractLeft;
            }
            // 右裁剪
            if p_start > start && p_end >= end {
                return SubtractResult::SubtractRight;
            }
            unreachable!();
        }
        let mut preserved = PRESERVE_MEMORY_REGIONS;
        preserved.sort();
        let mut start = region.start;
        let end = region.end;
        for (p_start, p_end) in preserved {
            match region_subtract(start, end, p_start, p_end) {
                SubtractResult::NoSubtract if region.end <= p_start => break,
                SubtractResult::NoSubtract => continue,
                SubtractResult::SubtractAll => return,
                SubtractResult::SubtractCenter => {
                    if let Some(left) = UsableMemoryRegion::from(start, p_start) {
                        self.push(left);
                    }
                    start = p_end;
                }
                SubtractResult::SubtractLeft => start = p_end,
                SubtractResult::SubtractRight => {
                    if let Some(left) = UsableMemoryRegion::from(start, p_start) {
                        self.push(left);
                    }
                    return;
                }
            }
        }
        if let Some(r) = UsableMemoryRegion::from(start, end) {
            self.push(r);
        }
    }

    fn check(&self) -> KResult<()> {
        // check whether kernal space is in the usable memory
        let ks = *unsafe { KERNEL_ADDR.get() };
        let ke = ks + *unsafe { KERNEL_LEN.get() };
        let res = self.is_address_overlap(ks, ke);
        info!("is kernal overlap free space? {:?}", res);
        if res.is_some() {
            Err(KError::Overflow)
        } else {
            Ok(())
        }
    }

    /// The memory regions is increased
    fn init(&mut self, data: &MemoryRegions) {
        let mut temp: Option<(u64, u64)> = None;
        let data = data.as_ref();
        let mut i = 0;
        while i < data.len() {
            match (data[i].kind, &mut temp) {
                (MemoryRegionKind::Usable, None) => temp = Some((data[i].start, data[i].end)),
                (MemoryRegionKind::Usable, Some((_, end))) if data[i].start == *end => {
                    *end = data[i].end
                }
                (MemoryRegionKind::Usable, Some((start, end))) => {
                    if let Some(r) = UsableMemoryRegion::from(*start, *end) {
                        self.push_sub_preserved(r);
                    }
                    *start = data[i].start;
                    *end = data[i].end;
                }
                (_, Some((start, end))) => {
                    if let Some(r) = UsableMemoryRegion::from(*start, *end) {
                        self.push_sub_preserved(r);
                    }
                    temp = None;
                }
                _ => {}
            }
            i += 1;
        }
        if let Some((start, end)) = temp
            && let Some(r) = UsableMemoryRegion::from(start, end)
        {
            self.push_sub_preserved(r);
        }

        let sum = self
            .usable_regions()
            .iter()
            .map(|v| v.end - v.start)
            .sum::<u64>();
        info!("{:?}", self.usable_regions());
        info!("total 0x{:X?} bytes free memory", sum);
    }
}

impl Debug for UsableMemoryRegions {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "UsableMemoryRegions {{ regions: {:?}, size: {} }}",
            &self.regions[..self.size],
            self.size
        )
    }
}
