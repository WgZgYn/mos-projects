use crate::cpu_info::{CpuId, MAX_CPUS};
use crate::gdt::{Selectors, init_gdt, init_tss, load};
use crate::sync::IrqLocked;
use crate::{init_fpu, init_gs, init_syscall};
use core::mem::{MaybeUninit, offset_of};
use core::sync::atomic::{AtomicU64, AtomicUsize};
use log::info;
use mos_lib::sync::DummyLock;
use x2apic::lapic::LocalApic;
use x86_64::VirtAddr;
use x86_64::registers::model_specific::GsBase;
use x86_64::structures::gdt::GlobalDescriptorTable;
use x86_64::structures::tss::TaskStateSegment;

const STACK_SIZE: usize = 4096 * 4;

#[repr(C)]
#[derive(Debug)]
pub struct PerCpu {
    pub kernel_stack_top: *mut u64,
    pub temp_rsp: *mut u64,
    pub current_tcb: *const u64,
    pub idle_tcb: *const u64,
    pub cpu_id: usize,
    pub tick: AtomicU64,
    pub lapic: MaybeUninit<LocalApic>,
    pub preempt_count: AtomicUsize,
    pub gdt: GlobalDescriptorTable,
    pub selectors: Selectors,
    pub tss: TaskStateSegment,
    pub double_fault_stack: [u8; STACK_SIZE],
}

impl Default for PerCpu {
    fn default() -> Self {
        Self::new()
    }
}

impl PerCpu {
    pub const fn new() -> Self {
        Self {
            kernel_stack_top: core::ptr::null_mut(),
            temp_rsp: core::ptr::null_mut(),
            current_tcb: core::ptr::null(),
            idle_tcb: core::ptr::null(),
            cpu_id: 0,
            tick: AtomicU64::new(0),
            lapic: MaybeUninit::uninit(),
            preempt_count: AtomicUsize::new(0),
            gdt: GlobalDescriptorTable::empty(),
            selectors: Selectors::dangling(),
            tss: TaskStateSegment::new(),
            double_fault_stack: [0; STACK_SIZE],
        }
    }

    pub fn update_rsp0(&mut self, rsp: VirtAddr) {
        self.tss.privilege_stack_table[0] = rsp;
    }
}

unsafe impl Sync for PerCpu {}
unsafe impl Send for PerCpu {}

pub static PER_CPU: DummyLock<[IrqLocked<PerCpu>; MAX_CPUS]> =
    unsafe { DummyLock::new([const { IrqLocked::new(PerCpu::new()) }; MAX_CPUS]) };

pub const KERNEL_STACK_OFFSET: usize = offset_of!(PerCpu, kernel_stack_top);
pub const TEMP_RSP_OFFSET: usize = offset_of!(PerCpu, temp_rsp);

pub unsafe fn access_mut<'a>(cpu_id: CpuId, f: impl FnOnce(&'a mut PerCpu)) {
    unsafe { PER_CPU.access(|cpus| cpus[cpu_id.as_usize()].access(f)) }
}

pub(crate) fn init(cpu_id: CpuId) {
    unsafe {
        let cpuid = cpu_id.as_usize();
        info!("[percpu][{:?}] init start, cpu index={}", cpu_id, cpuid);
        access_mut(cpu_id, |cpu| {
            let addr = VirtAddr::from_ptr(cpu);
            let PerCpu {
                gdt,
                tss,
                selectors,
                double_fault_stack,
                cpu_id,
                ..
            } = cpu;
            *cpu_id = cpuid;
            info!("[percpu][{:?}] setting up TSS + GDT...", cpu_id);
            init_tss(tss, VirtAddr::from_ptr(double_fault_stack));
            let ret = init_gdt(gdt, tss);
            load(gdt, &ret);
            *selectors = ret;
            info!("[percpu][{:?}] GDT loaded, setting GS base to {:#x}", cpu_id, addr.as_u64());
            init_gs(addr);
            info!("[percpu][{:?}] GS base set, initializing FPU...", cpu_id);
            init_fpu();
            info!("[percpu][{:?}] FPU done, initializing syscall MSRs...", cpu_id);
            init_syscall(selectors);
            info!("[percpu][{:?}] init complete", cpu_id);
        })
    }
}

pub unsafe fn current_raw() -> &'static PerCpu {
    unsafe { &*GsBase::read().as_ptr() }
}

pub unsafe fn current_raw_mut() -> &'static mut PerCpu {
    unsafe { &mut *GsBase::read().as_mut_ptr() }
}

pub fn is_gs_initialized() -> bool {
    GsBase::read().as_u64() != 0
}

pub fn try_current_raw() -> Option<&'static PerCpu> {
    if is_gs_initialized() {
        Some(unsafe { current_raw() })
    } else {
        None
    }
}

pub fn try_current_raw_mut() -> Option<&'static mut PerCpu> {
    if is_gs_initialized() {
        Some(unsafe { current_raw_mut() })
    } else {
        None
    }
}
