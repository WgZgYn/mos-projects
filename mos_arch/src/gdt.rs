use crate::structs::ProcessContext;
use core::mem::offset_of;
use core::ops::Deref;
use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::registers::rflags::RFlags;
use x86_64::registers::segmentation::{DS, SS};
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

const STACK_SIZE: usize = 4096 * 5;
static mut KERNEL_STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        let stack_start = VirtAddr::from_ptr(unsafe { &raw mut KERNEL_STACK });
        tss.privilege_stack_table[0] = stack_start + STACK_SIZE as u64;

        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe { &raw mut STACK });
            stack_start + STACK_SIZE as u64
        };
        tss
    };
}

#[derive(Debug)]
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_data: SegmentSelector,
    pub user_code: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

lazy_static! {
    pub static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (
            gdt,
            Selectors {
                kernel_code,
                kernel_data,
                user_code,
                user_data,
                tss_selector,
            },
        )
    };
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();

    unsafe {
        CS::set_reg(GDT.1.kernel_code);
        SS::set_reg(GDT.1.kernel_data);
        DS::set_reg(GDT.1.kernel_data);
        load_tss(GDT.1.tss_selector);
    }
}

/// # Safety
/// kernel rsp must align to 16, need sync
pub unsafe fn write_rsp0(rsp: VirtAddr) {
    unsafe {
        let ptr = TSS.deref() as *const _ as *const u8;
        ptr.add(offset_of!(TaskStateSegment, privilege_stack_table))
            .cast::<VirtAddr>()
            .cast_mut()
            .write_unaligned(rsp);
    }
}

impl ProcessContext {
    pub const DEFAULT_RFLAGS: RFlags = RFlags::INTERRUPT_FLAG; // IF=1
    pub fn kernel_context() -> Self {
        Self {
            cs: GDT.1.kernel_code,
            rflags: Self::DEFAULT_RFLAGS,
            ss: GDT.1.kernel_data,
        }
    }

    pub fn user_context() -> Self {
        Self {
            cs: GDT.1.user_code,
            rflags: Self::DEFAULT_RFLAGS,
            ss: GDT.1.user_data,
        }
    }
}
