#![no_std]

extern crate alloc;

pub mod acpi;
mod acpi_handler;
pub mod apic;
pub mod keyboard;
pub mod pic;
pub mod timer;

pub trait InterruptController {
    fn initialize(&self);
    fn enable_irq(&self, irq: u8);
    fn disable_irq(&self, irq: u8);
    fn eoi(&self, irq: u8);
}

pub const OS_HZ: u64 = 1000;

static INTERRUPT_CONTROLLER: apic::Apic = apic::apic_mode();
pub fn init() {
    INTERRUPT_CONTROLLER.initialize();
}
pub fn enable_irq(irq: u8) {
    INTERRUPT_CONTROLLER.enable_irq(irq)
}
pub fn disable_irq(irq: u8) {
    INTERRUPT_CONTROLLER.disable_irq(irq)
}
pub fn eoi(irq: u8) {
    INTERRUPT_CONTROLLER.eoi(irq)
}
