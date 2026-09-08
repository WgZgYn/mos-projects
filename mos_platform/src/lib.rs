#![no_std]

extern crate alloc;
pub mod acpi;
mod acpi_handler;
pub mod apic;
pub mod keyboard;
pub mod pic;
pub mod rtc;
pub mod smp;
pub mod timer;

use crate::acpi::{APIC, init_lapic};
use core::sync::atomic::{AtomicBool, Ordering};
use log::info;
use mos_arch::cpu_info::{CpuId, CpuState};
use mos_arch::percpu::{current_raw, current_raw_mut};
use mos_driver::{println, serial_println};
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
const TEMP_PML4_ADDR: u64 = 0x6000;

/// 设为 true 让 AP 进入诊断循环（不参与调度），验证 LAPIC timer 是否正常
const AP_DIAG_MODE: bool = false;

static INTERRUPT_CONTROLLER: apic::Apic = apic::apic_mode();

pub trait InterruptController {
    fn initialize(&self);
    fn enable_irq(&self, irq: u8);
    fn disable_irq(&self, irq: u8);
    fn eoi(&self, irq: u8);
}

fn read_lvt_timer() -> u64 {
    if is_x2apic_enabled() {
        unsafe { x86_64::registers::model_specific::Msr::new(0x832).read() }
    } else {
        let xapic_base = APIC.get().unwrap().xapic_base;
        unsafe { core::ptr::read_volatile((xapic_base + 0x320) as *const u32) as u64 }
    }
}

fn ap_diagnostic_loop() -> ! {
    use core::sync::atomic::Ordering;
    use crate::acpi::{PM_TIMER_HZ, pm_timer_elapsed_since, pm_timer_tick};

    let cpu_id = unsafe { current_raw().cpu_id };
    x86_64::instructions::interrupts::enable();

    let mut last_tick = 0u64;
    let mut round = 0u64;
    const ROUNDS_PER_PRINT: u64 = 10; // 每 10 轮 (~10秒) 打印一次心跳
    const PM_WAIT_TICKS: u64 = PM_TIMER_HZ; // PM timer 1秒间隔

    loop {
        let percpu = unsafe { current_raw() };
        let tick = percpu.tick.load(Ordering::Relaxed);

        if tick != last_tick {
            let lapic = unsafe { percpu.lapic.assume_init_ref() };
            let tccr = unsafe { lapic.timer_current() };
            let apic_id = unsafe { lapic.id() };
            let lvt = read_lvt_timer();
            println!("========================================");
            println!("AP{} TICK! {}->{} tccr={} id={} lvt={:x}",
                cpu_id, last_tick, tick, tccr, apic_id, lvt);
            println!("========================================");
            last_tick = tick;
        } else if round % ROUNDS_PER_PRINT == 0 {
            let lapic = unsafe { percpu.lapic.assume_init_ref() };
            let tccr = unsafe { lapic.timer_current() };
            let lvt = read_lvt_timer();
            println!("AP{} r{} tick={} tccr={} lvt={:x}",
                cpu_id, round, tick, tccr, lvt);
        }

        round += 1;
        // 忙等约 1 秒 (PM timer 精确计时), 中断可在此期间响应
        let start = pm_timer_tick();
        while pm_timer_elapsed_since(start) < PM_WAIT_TICKS as u32 {
            core::hint::spin_loop();
        }
    }
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
    info!(
        "[AP {:>2}] trampoline ok, cr3={:#x}",
        boot_data.cpu_id.as_usize(), boot_data.cr3
    );
    mos_arch::idt::load_idt();
    post_init(boot_data.cpu_id);
    // signal BSP BEFORE entering first_schedule (which never returns)
    boot_data.finish.store(true, Ordering::Release);
    info!("[AP {:>2}] init done, entering idle loop", boot_data.cpu_id.as_usize());
    first_schedule()
}

pub fn post_init(id: CpuId) {
    mos_arch::init_percpu(id);
    unsafe {
        let percpu = current_raw_mut();
        let lapic = init_lapic();
        let if_on = x86_64::instructions::interrupts::are_enabled();
        info!(
            "[AP {:>2}] LAPIC ready, apic_id={}, intrs_enabled={}",
            id.as_usize(),
            lapic.id(),
            if_on
        );
        percpu.lapic.write(lapic);
        let is_ap = id.as_usize() != 0;
        let idle_func = if AP_DIAG_MODE && is_ap { ap_diagnostic_loop } else { sti_hlt_loop };
        if AP_DIAG_MODE && is_ap {
            println!("[AP DIAG] cpu {} using diagnostic idle loop", id.as_usize());
        }
        let idle_task = ProcessControlBlock::kernel_func(KERNEL.clone(), idle_func, Priority(0));
        percpu.idle_tcb = KRef::into_ptr(idle_task).as_ptr() as _;
    }
}

pub const ENABLE_SMP: bool = cfg!(feature = "smp");

pub fn init() {
    info!("[INTR] MODE: APIC");
    INTERRUPT_CONTROLLER.initialize();

    if ENABLE_SMP {
        info!("[SMP] setting up AP trampoline...");
        setup_ap_trampoline();
        info!("[SMP] trampoline setup complete");
        mos_arch::cpu_info::access_mut(|cpus| {
            let ap_count = cpus.iter().filter(|c| !c.is_bsp && c.state == CpuState::Waiting).count();
            info!("[SMP] found {} AP(s) in Waiting state", ap_count);
            for cpu in cpus
                .iter_mut()
                .filter(|c| !c.is_bsp && c.state == CpuState::Waiting)
            {
                cpu.state = CpuState::Starting;
                info!(
                    "[SMP] starting AP {:?}, apic_id={}",
                    cpu.cpu_id, cpu.apic_id
                );
                serial_println!("[AP] {:?} starting, apic_id: {}", cpu.cpu_id, cpu.apic_id);

                let bootdata = setup_boot_data(cpu.cpu_id, ap_init as *const () as _);
                start_ap(
                    &mut APIC.get().expect("should init apic").bsp_lapic.lock(),
                    cpu.apic_id,
                );

                // wait for AP to finish with timeout (~1s in PM timer ticks)
                let start = acpi::pm_timer_tick();
                let timeout = acpi::PM_TIMER_HZ as u32; // 1 second
                while !bootdata.finish.load(Ordering::Acquire) {
                    if acpi::pm_timer_elapsed_since(start) > timeout {
                        let elapsed_ms = acpi::pm_timer_tick_to_ms(acpi::pm_timer_elapsed_since(start));
                        info!(
                            "[SMP][{:?}] TIMEOUT after {}ms (pm_ticks={})",
                            cpu.cpu_id,
                            elapsed_ms,
                            acpi::pm_timer_elapsed_since(start)
                        );
                        serial_println!(
                            "[AP] {:?} TIMEOUT after {}ms",
                            cpu.cpu_id,
                            acpi::pm_timer_tick_to_ms(timeout)
                        );
                        break;
                    }
                    core::hint::spin_loop();
                }

                if bootdata.finish.load(Ordering::Acquire) {
                    let elapsed_ms = acpi::pm_timer_tick_to_ms(acpi::pm_timer_elapsed_since(start));
                    cpu.state = CpuState::Running;
                    info!("[SMP][{:?}] AP init SUCCESS in {}ms", cpu.cpu_id, elapsed_ms);
                    serial_println!("[AP] {:?} init finish", cpu.cpu_id);
                } else {
                    cpu.state = CpuState::Disabled;
                    info!("[SMP][{:?}] AP init FAILED", cpu.cpu_id);
                    serial_println!("[AP] {:?} init FAILED", cpu.cpu_id);
                }
            }
        });
        info!("[SMP] AP bring-up complete");
    }
    info!("[SMP] initializing BSP percpu...");
    let bsp = mos_arch::cpu_info::access_mut(|cpus| {
        cpus.iter().filter(|c| c.is_bsp).map(|c| c.cpu_id).next()
    })
    .expect("should has bsp");
    info!("[SMP] BSP id={:?}, entering post_init", bsp);
    post_init(bsp);
    info!("[SMP] BSP post_init complete");

    // ── BOOT SUMMARY ──
    mos_arch::cpu_info::access_mut(|cpus| {
        let total = cpus.iter().count();
        let running = cpus.iter().filter(|c| c.state == CpuState::Running).count();
        let disabled = cpus.iter().filter(|c| c.state == CpuState::Disabled).count();
        let waiting = cpus.iter().filter(|c| c.state == CpuState::Waiting).count();
        let ap_ok = running.saturating_sub(1); // minus BSP
        let ap_total = total.saturating_sub(1);

        info!("");
        info!("============ MOS BOOT SUMMARY ============");
        info!("  Total CPUs: {}  |  Running: {}  |  Failed: {}  |  Unstarted: {}", total, running, disabled, waiting);
        info!("  BSP: 1  |  APs: {}/{}", ap_ok, ap_total);
        for cpu in cpus.iter() {
            let tag = if cpu.is_bsp { "BSP" } else { "AP" };
            let state_s = match cpu.state {
                CpuState::Running => "OK     ",
                CpuState::Disabled => "FAILED ",
                CpuState::Starting => "PENDING",
                CpuState::Waiting => "UNUSED ",
            };
            info!("  [{}] {} | apic_id={} | {}", tag, state_s, cpu.apic_id,
                if cpu.is_bsp { "boot processor" } else { "" });
        }
        info!("============================================");
        info!("");
    });
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

/// Raw write to ICR, bypassing the x2apic crate's format_icr (which always sets Level=1).
/// This is needed for INIT deassert (Level=0).
///
/// ICR layout (Intel SDM Vol.3 §10.6.1):
///   bits  0: 7  = Vector
///   bits  8:10  = Delivery Mode (5 = INIT, 6 = StartUp)
///   bit  11      = Destination Mode (0 = Physical)
///   bit  14      = Level (1 = Assert, 0 = Deassert)
///   bit  15      = Trigger Mode (1 = Level, 0 = Edge)
///   bits 56:63   = Destination APIC ID (xAPIC; x2APIC uses bits 32:63)
pub(crate) unsafe fn write_raw_icr(xapic_base: u64, icr: u64, x2apic: bool) {
    if x2apic {
        unsafe {
            x86_64::registers::model_specific::Msr::new(0x830).write(icr);
        }
    } else {
        // xAPIC: ICR high at base+0x310, ICR low at base+0x300
        unsafe {
            core::ptr::write_volatile((xapic_base + 0x310) as *mut u32, (icr >> 32) as u32);
            core::ptr::write_volatile((xapic_base + 0x300) as *mut u32, icr as u32);
        }
    }
}

/// Check if x2APIC mode is currently enabled (IA32_APIC_BASE bit 10).
pub(crate) fn is_x2apic_enabled() -> bool {
    use x86_64::registers::model_specific::Msr;
    unsafe { Msr::new(0x1B).read() & (1 << 10) != 0 }
}

fn start_ap(lapic: &mut LocalApic, apic_id: u32) {
    const INIT_ASSERT_US: u32 = 200;
    const INIT_DEASSERT_MS: u32 = 10;
    const SIPI_DELAY_US: u32 = 200;
    const SIPI_RETRY: u32 = 2;

    let x2apic = is_x2apic_enabled();
    let xapic_base = APIC.get().unwrap().xapic_base;

    info!("[SMP][apic={}] flushing caches (wbinvd)...", apic_id);
    #[cfg(target_arch = "x86_64")]
    unsafe {
        core::arch::asm!("wbinvd", options(nostack));
    }

    // wait for ICR idle
    let mut spin = 0u32;
    while unsafe { lapic.get_ipi_delivery_status() } {
        spin += 1;
        core::hint::spin_loop();
    }

    // ── INIT ASSERT ──
    // ICR: DeliveryMode=INIT(5), Level=1, Trigger=Level(1), Physical
    let dest: u64 = if x2apic { (apic_id as u64) << 32 } else { (apic_id as u64) << 56 };
    let icr_assert: u64 = dest | (5 << 8) | (1 << 14) | (1 << 15);

    info!(
        "[SMP][apic={}] sending INIT ASSERT (x2apic={}, icr={:#x})...",
        apic_id, x2apic, icr_assert
    );
    unsafe {
        write_raw_icr(xapic_base, icr_assert, x2apic);
    }

    // wait for delivery
    spin = 0;
    while unsafe { lapic.get_ipi_delivery_status() } {
        spin += 1;
        if spin.is_multiple_of(1_000_000) {
            info!(
                "[SMP][apic={}] still waiting for INIT ASSERT delivery (spins={})...",
                apic_id, spin
            );
        }
        core::hint::spin_loop();
    }
    info!("[SMP][apic={}] INIT ASSERT delivered ({} spins)", apic_id, spin);

    // ── brief gap between assert and deassert ──
    let us_delay = acpi::PM_TIMER_HZ as u32 / (1000 * 1000 / INIT_ASSERT_US);
    acpi::pm_sleep(us_delay);

    // ── INIT DEASSERT ──
    // ICR: same as assert but Level=0
    let icr_deassert: u64 = dest | (5 << 8) | (0 << 14) | (1 << 15);
    info!(
        "[SMP][apic={}] sending INIT DEASSERT (icr={:#x})...",
        apic_id, icr_deassert
    );
    unsafe {
        write_raw_icr(xapic_base, icr_deassert, x2apic);
    }

    spin = 0;
    while unsafe { lapic.get_ipi_delivery_status() } {
        spin += 1;
        if spin.is_multiple_of(1_000_000) {
            info!(
                "[SMP][apic={}] still waiting for INIT DEASSERT delivery (spins={})...",
                apic_id, spin
            );
        }
        core::hint::spin_loop();
    }
    info!("[SMP][apic={}] INIT DEASSERT delivered ({} spins)", apic_id, spin);

    // Intel SDM: 10ms between INIT deassert and first SIPI
    let pm_tick_dura = acpi::PM_TIMER_HZ as u32 / (1000 / INIT_DEASSERT_MS);
    info!(
        "[SMP][apic={}] waiting {}ms after deassert...",
        apic_id, INIT_DEASSERT_MS
    );
    acpi::pm_sleep(pm_tick_dura);

    // ── SIPIs ──
    for i in 0..SIPI_RETRY {
        while unsafe { lapic.get_ipi_delivery_status() } {
            core::hint::spin_loop();
        }

        let vector = (TRAMPOLINE_ADDR >> 12) as u8;
        info!(
            "[SMP][apic={}] sending SIPI #{} (vector={:#x}, addr={:#x})...",
            apic_id, i + 1, vector, TRAMPOLINE_ADDR
        );
        unsafe {
            // x2APIC: destination in bits 32-63, use raw apic_id
            // xAPIC: destination in bits 56-63 (byte at bits 24-31 of ICR high), shift by 24
            lapic.send_sipi(vector, if x2apic { apic_id } else { apic_id << 24 });
        }

        spin = 0;
        while unsafe { lapic.get_ipi_delivery_status() } {
            spin += 1;
            core::hint::spin_loop();
        }
        info!("[SMP][apic={}] SIPI #{} delivered ({} spins)", apic_id, i + 1, spin);

        if i + 1 < SIPI_RETRY {
            let delay = acpi::PM_TIMER_HZ as u32 / (1000 * 1000 / SIPI_DELAY_US);
            acpi::pm_sleep(delay);
        }
    }

    info!("[SMP][apic={}] INIT+SIPI sequence complete, BSP waiting", apic_id);
}

impl BootData {
    fn new(cpu_id: CpuId, ap_main: u64, stack_top: u64) -> Self {
        let cr3 = Cr3::read().0.start_address().as_u64();
        let cr0 = Cr0::read().bits();
        let cr4 = Cr4::read().bits();
        Self {
            cr3,
            stack_top,
            ap_main,
            bsp_cr0: cr0,
            bsp_cr4: cr4,
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
    info!(
        "[SMP] trampoline code size={} bytes, target phys={:#x}",
        CODE.len(),
        TRAMPOLINE_ADDR
    );
    unsafe {
        // identity mappings — must be created BEFORE copying PML4,
        // so the temp PML4 at 0x6000 captures all page table changes
        info!("[SMP] creating identity mappings...");
        make_identity_mapping(TEMP_PML4_ADDR, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        info!("[SMP]   TEMP_PML4  {:#x} -> {:#x}", TEMP_PML4_ADDR, TEMP_PML4_ADDR);
        make_identity_mapping(BOOT_DATA_ADDR, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
        info!("[SMP]   BOOT_DATA  {:#x} -> {:#x}", BOOT_DATA_ADDR, BOOT_DATA_ADDR);
        make_identity_mapping(TRAMPOLINE_ADDR, PageTableFlags::PRESENT);
        info!("[SMP]   TRAMPOLINE {:#x} -> {:#x}", TRAMPOLINE_ADDR, TRAMPOLINE_ADDR);
        make_identity_mapping(
            TEMP_STACK_START,
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
        info!("[SMP]   TEMP_STACK {:#x} -> {:#x}", TEMP_STACK_START, TEMP_STACK_START);

        // copy BSP's PML4 directly to 0x6000 physical page
        let bsp_pml4_phys = Cr3::read().0.start_address().as_u64();
        info!(
            "[SMP] copying BSP PML4 (phys={:#x}) -> TEMP_PML4 (phys={:#x})",
            bsp_pml4_phys, TEMP_PML4_ADDR
        );
        let bsp_pml4_virt = phys_to_virt(PhysAddr::new(bsp_pml4_phys));
        let temp_pml4_virt = phys_to_virt(PhysAddr::new(TEMP_PML4_ADDR));
        core::ptr::copy_nonoverlapping::<u8>(
            bsp_pml4_virt.as_ptr(),
            temp_pml4_virt.as_mut_ptr(),
            4096,
        );
        info!("[SMP] PML4 copy done");
        info!("[SMP] writing trampoline code to phys {:#x}...", TRAMPOLINE_ADDR);
        mos_mem::write_to_phys(PhysAddr::new(TRAMPOLINE_ADDR), CODE);
        info!("[SMP] trampoline code written");
    };
}

// TODO: free or reuse the boot stack
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
