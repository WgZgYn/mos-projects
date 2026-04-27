#![no_std]

use crate::asm::syscall_entry_point;
use crate::cpu_info::CpuId;
use crate::gdt::Selectors;
use crate::structs::FpuState;
use core::arch::asm;
use core::arch::x86_64::_rdtsc;
use lazy_static::lazy_static;
use x86_64::VirtAddr;
use x86_64::instructions::random::RdRand;
use x86_64::registers::control::{Cr0, Cr0Flags, Cr4, Cr4Flags};
use x86_64::registers::model_specific::{GsBase, KernelGsBase};
use x86_64::registers::mxcsr;
use x86_64::registers::mxcsr::MxCsr;
use x86_64::structures::paging::PhysFrame;
pub mod asm;
pub mod cpu_info;
pub mod gdt;
pub mod idt;
pub mod percpu;
pub mod rand;
pub mod structs;
pub mod sync;

#[derive(Debug)]
pub struct CpuInfo {
    pub base_frequency: Option<u16>, // MHz
    pub max_frequency: Option<u16>,  // MHz
    pub bus_frequency: Option<u16>,  // MHz
}

#[derive(Debug)]
pub struct CpuTscInfo {
    pub has_tsc: Option<bool>,
    pub tsc_freq: Option<u64>,
    pub has_invariant_tsc: Option<bool>,
}

pub fn cpu_tsc_info() -> CpuTscInfo {
    let cpuid = raw_cpuid::CpuId::new();
    let has_tsc = cpuid.get_feature_info().map(|finfo| finfo.has_tsc());
    let has_invariant_tsc = cpuid
        .get_advanced_power_mgmt_info()
        .map(|adfo| adfo.has_invariant_tsc());
    let tsc_freq = cpuid.get_tsc_info().and_then(|tinfo| tinfo.tsc_frequency());

    CpuTscInfo {
        has_tsc,
        tsc_freq,
        has_invariant_tsc,
    }
}

pub fn rand() -> Option<u64> {
    lazy_static! {
        static ref RAND: Option<RdRand> = RdRand::new();
    }
    RAND.and_then(|r| r.get_u64())
}

pub fn tick() -> u64 {
    unsafe { _rdtsc() }
}

pub fn hlt() {
    x86_64::instructions::hlt()
}

pub fn enable_and_halt() {
    x86_64::instructions::interrupts::enable_and_hlt();
}

pub fn nop() {
    x86_64::instructions::nop()
}

pub fn soft_interrupt<const N: u8>() {
    unsafe { x86_64::instructions::interrupts::software_interrupt::<N>() }
}

pub fn breakpoint() {
    x86_64::instructions::interrupts::int3();
}

pub fn is_enable_interrupts() -> bool {
    x86_64::instructions::interrupts::are_enabled()
}

pub fn enable_interrupts() {
    x86_64::instructions::interrupts::enable();
}

pub fn disable_interrupts() {
    x86_64::instructions::interrupts::disable();
}

pub fn with_disabled_interrupts<R, F: FnOnce() -> R>(f: F) -> R {
    x86_64::instructions::interrupts::without_interrupts(f)
}

pub fn page_table_base_frame() -> PhysFrame {
    x86_64::registers::control::Cr3::read().0
}

pub fn init_fpu() {
    let mut cr0 = Cr0::read();
    cr0 -= Cr0Flags::EMULATE_COPROCESSOR;
    cr0 |= Cr0Flags::MONITOR_COPROCESSOR;
    cr0 -= Cr0Flags::TASK_SWITCHED;
    unsafe {
        Cr0::write(cr0);
    }
    let mut cr4 = Cr4::read();
    cr4 = cr4 | Cr4Flags::OSFXSR | Cr4Flags::OSXMMEXCPT_ENABLE;
    unsafe {
        Cr4::write(cr4);

        asm!("fninit", options(nostack, preserves_flags));
        mxcsr::write(MxCsr::from_bits_truncate(0x1F80));
    }
}

pub fn init_fpu_state(state: &mut FpuState) {
    unsafe {
        asm!("fninit", options(nostack, preserves_flags));
        asm!("fxsave64 [{}]", in(reg) state as *mut FpuState, options(nostack, preserves_flags));
    }
}

pub fn save_fpu_state(state: &mut FpuState) {
    unsafe {
        asm!("fxsave64 [{}]", in(reg) state as *mut FpuState, options(nostack, preserves_flags));
    }
}

pub fn load_fpu_state(state: &mut FpuState) {
    unsafe {
        asm!("fxrstor64 [{}]", in(reg) state as *mut FpuState, options(nostack, preserves_flags));
    }
}

fn init_gs(percpu: VirtAddr) {
    KernelGsBase::write(percpu);
    GsBase::write(percpu);
}

fn init_syscall(selectors: &Selectors) {
    use x86_64::registers::model_specific::{Efer, EferFlags, LStar, SFMask, Star};
    unsafe {
        Efer::update(|e| e.insert(EferFlags::SYSTEM_CALL_EXTENSIONS));
        Star::write(
            selectors.user_code,
            selectors.user_data,
            selectors.kernel_code,
            selectors.kernel_data,
        )
        .expect("failed to set star");
        LStar::write(VirtAddr::new(syscall_entry_point()));
        SFMask::write(x86_64::registers::rflags::RFlags::INTERRUPT_FLAG);
    }
}

pub fn init_percpu(cpu_id: CpuId) {
    percpu::init(cpu_id);
}

