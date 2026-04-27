use crate::structs::CpuContext;
use x86_64::instructions::segmentation::CS;
use x86_64::instructions::tables::load_tss;
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::{DS, SS, Segment};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::{PrivilegeLevel, VirtAddr};

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

#[derive(Copy, Clone, Debug)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

impl Selectors {
    pub const fn dangling() -> Self {
        Self {
            kernel_code: SegmentSelector(0),
            kernel_data: SegmentSelector(0),
            user_data: SegmentSelector(0),
            user_code: SegmentSelector(0),
            tss_selector: SegmentSelector(0),
        }
    }
}

pub fn init_gdt(gdt: &mut GlobalDescriptorTable, tss: &'static TaskStateSegment) -> Selectors {
    let kernel_code = gdt.append(Descriptor::kernel_code_segment());
    let kernel_data = gdt.append(Descriptor::kernel_data_segment());
    let user_data = gdt.append(Descriptor::user_data_segment());
    let user_code = gdt.append(Descriptor::user_code_segment());
    let tss_selector = gdt.append(Descriptor::tss_segment(tss));
    Selectors {
        kernel_code,
        kernel_data,
        user_code,
        user_data,
        tss_selector,
    }
}

pub fn init_tss(tss: &mut TaskStateSegment, double_fault_stack: VirtAddr) {
    tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = double_fault_stack;
}

pub fn update_rsp0(tss: &mut TaskStateSegment, rsp: VirtAddr) {
    tss.privilege_stack_table[0] = rsp;
}

pub fn load(gdt: &'static GlobalDescriptorTable, selectors: &Selectors) {
    gdt.load();
    unsafe {
        CS::set_reg(selectors.kernel_code);
        SS::set_reg(selectors.kernel_data);
        DS::set_reg(selectors.kernel_data);
        load_tss(selectors.tss_selector);
    }
}

impl CpuContext {
    pub const DEFAULT_RFLAGS: RFlags = RFlags::INTERRUPT_FLAG;
    pub const fn kernel_context() -> Self {
        Self {
            cs: SegmentSelector::new(1, PrivilegeLevel::Ring0),
            ss: SegmentSelector::new(2, PrivilegeLevel::Ring0),
            rflags: Self::DEFAULT_RFLAGS,
        }
    }
    pub const fn user_context() -> Self {
        Self {
            cs: SegmentSelector::new(4, PrivilegeLevel::Ring3),
            ss: SegmentSelector::new(3, PrivilegeLevel::Ring3),
            rflags: Self::DEFAULT_RFLAGS,
        }
    }
}
