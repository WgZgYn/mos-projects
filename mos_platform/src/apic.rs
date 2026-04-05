use crate::acpi::{disable_irq, enable_irq, eoi};
use crate::InterruptController;

pub struct Apic;

impl InterruptController for Apic {
    fn initialize(&self) {}
    fn enable_irq(&self, irq: u8) {
        enable_irq(irq);
    }
    fn disable_irq(&self, irq: u8) {
        disable_irq(irq);
    }
    fn eoi(&self, _irq: u8) {
        eoi()
    }
}

pub const fn apic_mode() -> Apic {
    Apic
}
  