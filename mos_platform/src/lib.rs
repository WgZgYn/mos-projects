#![no_std]

extern crate alloc;
pub mod acpi;
mod acpi_handler;
pub mod apic;
pub mod keyboard;
pub mod pic;
pub mod timer;

use crate::acpi::{APIC, init_lapic};
use core::sync::atomic::{AtomicBool, Ordering};
use log::info;
use mos_arch::cpu_info::CpuId;
use mos_arch::percpu::current_raw_mut;
use mos_driver::serial_println;
use mos_lib::sync::KRef;
use mos_mem::{PhysicalFrameManager, phys_to_virt};
use mos_task::first_schedule;
use mos_task::process::{KERNEL, ProcessControlBlock};
use mos_task::thread::{Priority, sti_hlt_loop};
use x2apic::lapic::LocalApic;
use x86_64::registers::control::{Cr0, Cr3, Cr4};
use x86_64::structures::paging::{Mapper, Page, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

pub const OS_HZ: u64 = 1000;
const BOOT_DATA_ADDR: u64 = 0x7000;
const TRAMPOLINE_ADDR: u64 = 0x8000;
const TEMP_STACK_START: u64 = 0x9000;
const TEMP_STACK_TOP: u64 = 0xA000;

static INTERRUPT_CONTROLLER: apic::Apic = apic::apic_mode();

pub trait InterruptController {
    fn initialize(&self);
    fn enable_irq(&self, irq: u8);
    fn disable_irq(&self, irq: u8);
    fn eoi(&self, irq: u8);
}

#[repr(C)]
#[derive(Debug)]
struct BootData {
    cr3: u64,
    stack_top: u64,
    ap_main: u64,
    bsp_cr0: u64,
    bsp_cr4: u64,
    cpu_id: CpuId,
    finish: AtomicBool,
}

fn ap_init(boot_data: &BootData) {
    serial_println!("[AP] {:?} init start", boot_data.cpu_id);
    mos_arch::idt::load_idt();
    post_init(boot_data.cpu_id);
    boot_data.finish.store(true, Ordering::Release);
    serial_println!("[AP] {:?} first schedule", boot_data.cpu_id);
    first_schedule()
}

pub fn post_init(id: CpuId) {
    serial_println!("post init of {:?}", id);
    mos_arch::init_percpu(id);
    // need register the percpu
    unsafe {
        let percpu = current_raw_mut();
        percpu.lapic.write(init_lapic());
        let idle_task = ProcessControlBlock::kernel_func(KERNEL.clone(), sti_hlt_loop, Priority(0));
        percpu.idle_tcb = KRef::into_ptr(idle_task).as_ptr() as _
    }
}

pub fn init() {
    info!(
        "[INTR] MODE: {}",
        if cfg!(feature = "intr_legacy") {
            "LEGACY"
        } else {
            "APIC"
        }
    );
    INTERRUPT_CONTROLLER.initialize();
    setup_ap_trampoline();
    mos_arch::cpu_info::access(|cpus| {
        for cpu in cpus.iter().filter(|c| !c.is_bsp) {
            let bootdata = setup_boot_data(cpu.cpu_id, ap_init as *const () as _);
            start_ap(
                &mut APIC.get().expect("should init apic").bsp_lapic.lock(),
                cpu.apic_id,
            );
            while !bootdata.finish.load(Ordering::Acquire) {
                core::hint::spin_loop();
            }
            serial_println!("[AP] {:?} init finish", cpu.cpu_id);
        }
    });
    let bsp = mos_arch::cpu_info::access(|cpus| {
        cpus.iter().filter(|c| c.is_bsp).map(|c| c.cpu_id).next()
    })
    .expect("should has bsp");
    post_init(bsp);
}

#[inline]
pub const fn ms_to_ticks(ms: u64) -> u64 {
    ms.saturating_mul(OS_HZ).div_ceil(1000)
}
#[inline]
pub const fn ticks_to_ms(ticks: u64) -> u64 {
    ticks.saturating_mul(1000) / OS_HZ
}
#[inline]
pub fn enable_irq(irq: u8) {
    INTERRUPT_CONTROLLER.enable_irq(irq)
}
#[inline]
pub fn disable_irq(irq: u8) {
    INTERRUPT_CONTROLLER.disable_irq(irq)
}
#[inline]
pub fn eoi(irq: u8) {
    INTERRUPT_CONTROLLER.eoi(irq)
}

fn start_ap(lapic: &mut LocalApic, apic_id: u32) {
    unsafe {
        let v = lapic.version();
        serial_println!("[ACPI] loca apic version: {}", v);
        serial_println!("[ACPI] copy_ap_trampoline");
        // copy ap trampoline

        serial_println!("[ACPI] send init ipi");
        // send init ipi
        lapic.send_init_ipi(apic_id << 24);

        serial_println!("[ACPI] delay 10ms");
        // delay 10ms
        let pm_tick_dura = acpi::PM_TIMER_HZ as u32 / (1000 / 10);
        acpi::pm_sleep(pm_tick_dura);

        serial_println!("[ACPI] send startup ipi");
        // send startup ipi
        lapic.send_sipi((TRAMPOLINE_ADDR >> 12) as u8, apic_id << 24);

        serial_println!("[ACPI] delay 200us");
        // delay 200us
        let pm_tick_dura = acpi::PM_TIMER_HZ as u32 / (1000 * 1000 / 200);
        acpi::pm_sleep(pm_tick_dura);

        serial_println!("[ACPI] send startup ipi");
        // send startup ipi
        lapic.send_sipi((TRAMPOLINE_ADDR >> 12) as u8, apic_id << 24);

        serial_println!("[ACPI] BSP wait finish");
    }
}

impl BootData {
    fn new(cpu_id: CpuId, ap_main: u64, stack_top: u64) -> Self {
        Self {
            cr3: Cr3::read().0.start_address().as_u64(),
            stack_top,
            ap_main,
            bsp_cr0: Cr0::read().bits(),
            bsp_cr4: Cr4::read().bits(),
            cpu_id,
            finish: AtomicBool::new(false),
        }
    }
}

fn setup_ap_trampoline() {
    unsafe fn make_identity_mapping(addr: u64, flags: PageTableFlags) {
        unsafe {
            let mut mapper = mos_mem::PageTableManager::current_mapper();
            mapper
                .map_to(
                    Page::<Size4KiB>::containing_address(VirtAddr::new(addr)),
                    PhysFrame::containing_address(PhysAddr::new(addr)),
                    flags,
                    &mut PhysicalFrameManager,
                )
                .expect("should map bootdata ok")
                .flush();
        }
    }

    const CODE: &[u8] = include_bytes!("./trampoline.bin");
    if TRAMPOLINE_ADDR & 0xfff != 0 {
        panic!("ap trampoline is not aligned to 4KiB")
    }
    unsafe {
        // code
        make_identity_mapping(TRAMPOLINE_ADDR, PageTableFlags::PRESENT);

        // stack
        make_identity_mapping(
            TEMP_STACK_START,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );

        // bootdata
        make_identity_mapping(
            BOOT_DATA_ADDR,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );

        mos_mem::write_to_phys(PhysAddr::new(TRAMPOLINE_ADDR), CODE);
    };
}

fn setup_boot_data(cpu_id: CpuId, ap_main: u64) -> &'static mut BootData {
    if BOOT_DATA_ADDR & 0xfff != 0 {
        panic!("boot data is not aligned to 4KiB")
    }

    let frames = PhysicalFrameManager
        .global_allocate_frames(4)
        .expect("should allocate frames");
    let phys_addr = frames.end.start_address();
    let virt_addr = phys_to_virt(phys_addr);

    let bootdata = unsafe { &mut *(BOOT_DATA_ADDR as *mut BootData) };
    *bootdata = BootData::new(cpu_id, ap_main, virt_addr.as_u64());
    bootdata
}
