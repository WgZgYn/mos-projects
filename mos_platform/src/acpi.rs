use crate::OS_HZ;
use crate::acpi_handler::AcpiHandler;
use crate::timer::init_monotonic_clock;
use acpi::address::AddressSpace;
use acpi::platform::{AcpiPlatform, InterruptModel, PmTimer};
use acpi::sdt::madt::Madt;
use acpi::{AcpiTables, Handler};
use conquer_once::spin::OnceCell;
use log::info;
use mos_driver::serial_println;
use mos_lib::KernelBootGlobal;
use spin::Mutex;
use x2apic::ioapic::IoApic;
use x2apic::lapic::{LocalApic, TimerDivide, TimerMode};
use x86_64::structures::paging::OffsetPageTable;

fn load_tables<H: Handler>(rsdp: usize, handler: H) -> AcpiTables<H> {
    unsafe {
        AcpiTables::from_rsdp(handler, rsdp).expect("should be able to parse the rsdp tables!")
    }
}

fn platform_info<H: Handler>(tables: AcpiTables<H>, handler: H) -> AcpiPlatform<H> {
    AcpiPlatform::new(tables, handler).expect("should be able to load the model!")
}

struct Apic(IoApic, LocalApic);

unsafe impl Send for Apic {}
unsafe impl Sync for Apic {}
static APIC: OnceCell<Mutex<Apic>> = OnceCell::uninit();

pub fn init(rsdp: u64, mapper: &OffsetPageTable<'static>) {
    let h = AcpiHandler::new(mapper);
    let t = load_tables(rsdp as usize, h.clone());
    let i = platform_info(t, h);

    serial_println!("ACPI InterruptModel: \n{:?}", i.interrupt_model);
    serial_println!("ACPI ProcessorInfo: \n{:?}", i.processor_info);
    serial_println!("ACPI PowerProfile: \n{:?}", i.power_profile);
    serial_println!("ACPI PmTimer: \n{:?}", i.pm_timer);
    serial_println!("ACPI Tables DSDT: \n{:?}", i.tables.dsdt());
    serial_println!("ACPI Tables SSDTS: \n");
    i.tables.ssdts().for_each(|table| {
        serial_println!("{:?}", table);
    });

    let madt = i.tables.find_table::<Madt>();
    if let Some(madt) = madt {
        let p = madt.get();
        info!("{:x?}", p);
    }

    match i.interrupt_model {
        InterruptModel::Unknown => {
            panic!("Unknown interrupt model");
        }
        InterruptModel::Apic(apic) => {
            let pm_timer = i.pm_timer.expect("should be able to load the PM timer!");
            unsafe {
                PM_TIMER.init(|| PmTimerWrapper::new(pm_timer));
            }
            init_monotonic_clock();

            let lapic_addr = apic.local_apic_address;
            let ioapic_addr = apic.io_apics.first().expect("should has ioapic").address;
            let mut ioapic =
                unsafe { IoApic::new(ioapic_addr as u64 + mapper.phys_offset().as_u64()) };
            unsafe {
                ioapic.init(0x20);
                ioapic.enable_irq(1);
            }

            const CALIBRATION_TIMER_INITIAL: u32 = u32::MAX;
            // 10ms
            const CALIBRATION_TARGET_PM_TICKS: u64 = PM_TIMER_HZ / 1000 * 10;

            let mut lapic = x2apic::lapic::LocalApicBuilder::new()
                .timer_vector(0x20)
                .error_vector(0x22)
                .spurious_vector(0x23)
                .timer_mode(TimerMode::OneShot)
                .timer_divide(TimerDivide::Div1)
                .timer_initial(CALIBRATION_TIMER_INITIAL)
                .set_xapic_base(lapic_addr + mapper.phys_offset().as_u64())
                .build()
                .expect("should be able to build the lapic");

            unsafe {
                lapic.enable();
                let start = pm_timer_tick();
                while pm_timer_elapsed_since(start) < CALIBRATION_TARGET_PM_TICKS as u32 {
                    core::hint::spin_loop();
                }
                let pm_duration = pm_timer_elapsed_since(start);
                let lapic_timer_current = lapic.timer_current();
                let lapic_duration = CALIBRATION_TIMER_INITIAL - lapic_timer_current;
                // lapic timer start -> pm_timer start -> pm_timer stop -> lapic timer stop
                // lapic ticks time maybe a bit higher than pm ticks time
                info!(
                    "[PM Timer]: 10ms busy loop, pm_duration: {}, lapic_duration: {}",
                    pm_duration, lapic_duration
                );

                const LAPIC_TIMER_DIV: u64 = 1;
                let lapic_hz = PM_TIMER_HZ * lapic_duration as u64 / pm_duration as u64;
                let initial = lapic_hz / (OS_HZ * LAPIC_TIMER_DIV);
                if initial >= u32::MAX as u64 {
                    panic!("lapic hz too high!");
                }
                info!("[Lapic Timer]: initial {}", initial);
                lapic.set_timer_mode(TimerMode::Periodic);
                lapic.set_timer_divide(TimerDivide::Div1);
                lapic.set_timer_initial(initial as u32);
            }

            APIC.init_once(|| Mutex::new(Apic(ioapic, lapic)))
        }
        _ => {
            unreachable!()
        }
    }
}

pub fn eoi() {
    if let Ok(acpi) = APIC.try_get() {
        unsafe { acpi.lock().1.end_of_interrupt() }
    }
}

pub fn enable_irq(irq: u8) {
    if let Ok(acpi) = APIC.try_get() {
        unsafe { acpi.lock().0.enable_irq(irq) }
    }
}

pub fn disable_irq(irq: u8) {
    if let Ok(acpi) = APIC.try_get() {
        unsafe { acpi.lock().0.disable_irq(irq) }
    }
}

const PM_TIMER_HZ: u64 = 3579545;

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
