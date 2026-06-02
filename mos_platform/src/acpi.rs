use crate::OS_HZ;
use crate::acpi_handler::AcpiHandler;
use crate::timer::init_monotonic_clock;
use acpi::address::AddressSpace;
use acpi::platform::{AcpiPlatform, InterruptModel, PmTimer, ProcessorState};
use acpi::sdt::madt::Madt;
use acpi::{AcpiTables, Handler};
use conquer_once::spin::OnceCell;
use log::info;
use mos_arch::cpu_info::{CpuInfo, CpuState};
use mos_driver::serial_println;
use mos_lib::KernelBootGlobal;
use x2apic::ioapic::IoApic;
use x2apic::lapic::{LocalApic, TimerDivide, TimerMode};
use x86_64::registers::control::{Cr0, Cr3, Cr4, Efer};
use x86_64::structures::gdt::Descriptor;
use x86_64::structures::paging::OffsetPageTable;
use mos_arch::percpu::current_raw_mut;

fn load_tables<H: Handler>(rsdp: usize, handler: H) -> AcpiTables<H> {
    unsafe {
        AcpiTables::from_rsdp(handler, rsdp).expect("should be able to parse the rsdp tables!")
    }
}

fn platform_info<H: Handler>(tables: AcpiTables<H>, handler: H) -> AcpiPlatform<H> {
    AcpiPlatform::new(tables, handler).expect("should be able to load the model!")
}

pub struct Apic {
    pub ioapic: spin::Mutex<IoApic>,
    pub bsp_lapic: spin::Mutex<LocalApic>,
    pub xapic_base: u64,
}

unsafe impl Send for Apic {}
unsafe impl Sync for Apic {}
pub static APIC: OnceCell<Apic> = OnceCell::uninit();

fn init_cpus_info<H: Handler>(platform: &AcpiPlatform<H>) -> bool {
    let Some(info) = &platform.processor_info else {
        return false;
    };
    mos_arch::cpu_info::init(|cpus| {
        cpus.push_with_id(|id| CpuInfo {
            apic_id: info.boot_processor.local_apic_id,
            is_bsp: true,
            cpu_id: id,
            state: CpuState::Running,
        });
        for ap in &info.application_processors {
            cpus.push_with_id(|id| CpuInfo {
                apic_id: ap.local_apic_id,
                is_bsp: false,
                cpu_id: id,
                state: match ap.state {
                    ProcessorState::Disabled => CpuState::Disabled,
                    ProcessorState::WaitingForSipi => CpuState::Waiting,
                    ProcessorState::Running => CpuState::Running,
                },
            });
        }
    });
    true
}

fn log_bsp_context() {
    let cr0 = Cr0::read();
    let cr3 = Cr3::read().0.start_address();
    let cr4 = Cr4::read();
    let efer = Efer::read();

    serial_println!("[ACPI] cr0 0x{:x}", cr0);
    serial_println!("\tcr3 0x{:x}", cr3);
    serial_println!("\tcr4 0x{:x}", cr4);
    serial_println!("\tefer 0x{:x}", efer);
    serial_println!("\t64 KERNEL CODE: {:X?}", Descriptor::kernel_code_segment());
    serial_println!("\t64 KERNEL DATA: {:X?}", Descriptor::kernel_data_segment());
}

fn init_pm_timer(pm_timer: PmTimer) {
    unsafe {
        PM_TIMER.init(|| PmTimerWrapper::new(pm_timer));
    }
}

fn init_ioapic(ioapic_base: u64) -> IoApic {
    let mut ioapic = unsafe { IoApic::new(ioapic_base) };
    unsafe {
        ioapic.init(0x20);
        ioapic.enable_irq(1);  // keyboard
        ioapic.enable_irq(12); // PS/2 mouse
    }
    ioapic
}
fn init_lapic_inner(xapic_base: u64) -> LocalApic {
    const CALIBRATION_TIMER_INITIAL: u32 = u32::MAX;
    const CALIBRATION_TARGET_PM_TICKS: u64 = PM_TIMER_HZ / 1000 * 10;

    let mut lapic = x2apic::lapic::LocalApicBuilder::new()
        .timer_vector(0x20)
        .error_vector(0x22)
        .spurious_vector(0x23)
        .timer_mode(TimerMode::OneShot)
        .timer_divide(TimerDivide::Div1)
        .timer_initial(CALIBRATION_TIMER_INITIAL)
        .set_xapic_base(xapic_base)
        .build()
        .expect("should be able to build the lapic");

    unsafe {
        lapic.enable();
        // x2apic crate's disable_local_interrupt_pins() writes 0 to LINT0/LINT1,
        // leaving them unmasked with vector=0. Mask them properly.
        if crate::is_x2apic_enabled() {
            x86_64::registers::model_specific::Msr::new(0x835).write(1 << 16);
            x86_64::registers::model_specific::Msr::new(0x836).write(1 << 16);
        } else {
            core::ptr::write_volatile((xapic_base + 0x350) as *mut u32, 1 << 16);
            core::ptr::write_volatile((xapic_base + 0x360) as *mut u32, 1 << 16);
        }
        let start = pm_timer_tick();
        while pm_timer_elapsed_since(start) < CALIBRATION_TARGET_PM_TICKS as u32 {
            core::hint::spin_loop();
        }
        let pm_duration = pm_timer_elapsed_since(start);
        let lapic_timer_current = lapic.timer_current();
        let lapic_duration = CALIBRATION_TIMER_INITIAL - lapic_timer_current;
        const LAPIC_TIMER_DIV: u64 = 1;
        let lapic_hz = PM_TIMER_HZ * lapic_duration as u64 / pm_duration as u64;
        let initial = lapic_hz / (OS_HZ * LAPIC_TIMER_DIV);
        if initial >= u32::MAX as u64 {
            panic!("lapic hz too high! lapic_hz={}, initial={}", lapic_hz, initial);
        }
        lapic.set_timer_mode(TimerMode::Periodic);
        lapic.set_timer_divide(TimerDivide::Div1);
        lapic.set_timer_initial(initial as u32);
        // 显式清除 LVT Timer 的 Mask 位 (bit 16)
        // x2apic crate 的 set_timer_mode 在实机上可能不清除 mask 位，
        // 导致 timer 中断被屏蔽。QEMU 默认 mask=0 所以不受影响。
        if crate::is_x2apic_enabled() {
            let mut lvt = x86_64::registers::model_specific::Msr::new(0x832).read();
            lvt &= !(1u64 << 16);
            x86_64::registers::model_specific::Msr::new(0x832).write(lvt);
        } else {
            let mut lvt = core::ptr::read_volatile((xapic_base + 0x320) as *const u32);
            lvt &= !(1u32 << 16);
            core::ptr::write_volatile((xapic_base + 0x320) as *mut u32, lvt);
        }
        // verify: read back LVT timer + TCCR to confirm timer is properly configured
        let tccr1 = lapic.timer_current();
        // short spin to see if timer is counting down
        for _ in 0..1000 { core::hint::spin_loop(); }
        let tccr2 = lapic.timer_current();
        let lvt = if crate::is_x2apic_enabled() {
            x86_64::registers::model_specific::Msr::new(0x832).read()
        } else {
            core::ptr::read_volatile((xapic_base + 0x320) as *const u32) as u64
        };
        info!(
            "[LAPIC] apic_id={}, hz={}, period={}, tccr1={}, tccr2={}, lvt={:#x}",
            lapic.id(), lapic_hz, initial, tccr1, tccr2, lvt
        );
    }
    lapic
}

pub fn init_lapic() -> LocalApic {
    init_lapic_inner(APIC.get().unwrap().xapic_base)
}

pub fn init(rsdp: u64, mapper: &OffsetPageTable<'static>) {
    log_bsp_context();

    let handler = AcpiHandler::new(mapper);
    let tables = load_tables(rsdp as usize, handler.clone());
    let platform = platform_info(tables, handler);

    if !init_cpus_info(&platform) {
        serial_println!("[ACPI] init cpus info failed");
    }

    serial_println!("ACPI InterruptModel: {:?}", platform.interrupt_model);
    serial_println!("ACPI ProcessorInfo: {:?}", platform.processor_info);
    serial_println!("ACPI PowerProfile: {:?}", platform.power_profile);
    serial_println!("ACPI PmTimer: {:?}", platform.pm_timer);
    serial_println!("ACPI Tables DSDT: {:?}", platform.tables.dsdt());
    serial_println!("ACPI Tables SSDTS: ");
    platform.tables.ssdts().for_each(|table| {
        serial_println!("{:?}", table);
    });

    let InterruptModel::Apic(apic) = platform.interrupt_model else {
        panic!("Unsupported platform");
    };

    let pm_timer = platform
        .pm_timer
        .expect("should be able to load the PM timer!");

    init_pm_timer(pm_timer);
    init_monotonic_clock();

    let lapic_addr = apic.local_apic_address;
    let ioapic_addr = apic.io_apics.first().expect("should has ioapic").address;

    let ioapic_base = ioapic_addr as u64 + mapper.phys_offset().as_u64();
    let ioapic = init_ioapic(ioapic_base);

    let xapic_base = lapic_addr + mapper.phys_offset().as_u64();
    let lapic = init_lapic_inner(xapic_base);

    APIC.init_once(|| Apic {
        ioapic: spin::Mutex::new(ioapic),
        bsp_lapic: spin::Mutex::new(lapic),
        xapic_base,
    })
}

pub fn eoi() {
    if let Ok(acpi) = APIC.try_get() {
        // unsafe { acpi.bsp_lapic.lock().end_of_interrupt() }
        unsafe { current_raw_mut().lapic.assume_init_mut().end_of_interrupt() }
    }
}

pub fn enable_irq(irq: u8) {
    if let Ok(acpi) = APIC.try_get() {
        unsafe { acpi.ioapic.lock().enable_irq(irq) }
    }
}

pub fn disable_irq(irq: u8) {
    if let Ok(acpi) = APIC.try_get() {
        unsafe { acpi.ioapic.lock().disable_irq(irq) }
    }
}

pub const PM_TIMER_HZ: u64 = 3579545;

struct PmTimerWrapper(PmTimer);

impl PmTimerWrapper {
    fn new(timer: PmTimer) -> Self {
        Self(timer)
    }
    #[inline]
    fn read(&self) -> u32 {
        match self.0.base.address_space {
            AddressSpace::SystemIo => {
                let port = self.0.base.address as u16;
                unsafe { x86_64::instructions::port::Port::<u32>::new(port).read() }
            }
            _ => panic!("unsupported addressing space!"),
        }
    }

    #[inline]
    fn mask(&self) -> u32 {
        if self.0.supports_32bit {
            0xFFFF_FFFF
        } else {
            0x00FF_FFFF
        }
    }

    #[inline]
    fn elapsed(&self, start: u32, end: u32) -> u32 {
        let mask = self.mask();
        (end.wrapping_sub(start)) & mask
    }

    #[inline]
    fn elapsed_since(&self, start: u32) -> u32 {
        let now = self.read();
        self.elapsed(start, now)
    }
}

static PM_TIMER: KernelBootGlobal<PmTimerWrapper> = KernelBootGlobal::uninit();

#[inline]
pub fn pm_timer_tick() -> u32 {
    unsafe { PM_TIMER.get().read() }
}

#[inline]
pub const fn pm_timer_freq() -> u32 {
    PM_TIMER_HZ as u32
}

#[inline]
pub fn pm_timer_mask() -> u32 {
    unsafe { PM_TIMER.get().mask() }
}

#[inline]
pub fn pm_timer_elapsed_since(tick: u32) -> u32 {
    unsafe { PM_TIMER.get().elapsed_since(tick) }
}

#[inline]
pub fn pm_timer_tick_to_ms(tick_dura: u32) -> u32 {
    tick_dura * 1000 / pm_timer_freq()
}

pub fn pm_sleep(pm_tick_dura: u32) {
    let start = pm_timer_tick();
    while pm_timer_tick() < start + pm_tick_dura {
        core::hint::spin_loop();
    }
}
