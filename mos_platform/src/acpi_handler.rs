#![allow(unused)]

use acpi::aml::AmlError;
use acpi::{Handle, Handler, PciAddress, PhysicalMapping};
use core::ptr::NonNull;
use x86_64::structures::paging::OffsetPageTable;

#[derive(Clone)]
pub struct AcpiHandler<'a, 'b> {
    mapper: &'b OffsetPageTable<'a>,
}

impl<'a, 'b> AcpiHandler<'a, 'b> {
    pub fn new(mapper: &'b OffsetPageTable<'a>) -> Self {
        Self { mapper }
    }
}

impl<'a, 'b> Handler for AcpiHandler<'a, 'b> {
    unsafe fn map_physical_region<T>(
        &self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<Self, T> {
        let virt_addr = self.mapper.phys_offset().as_u64() as usize + physical_address;
        let ptr = NonNull::new(virt_addr as *mut T).unwrap();
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: ptr,
            region_length: size,
            mapped_length: size,
            handler: self.clone(),
        }
    }

    fn unmap_physical_region<T>(_region: &PhysicalMapping<Self, T>) {
        // 由于是恒定映射（identity mapping + offset），无需真正unmap
    }

    fn read_u8(&self, address: usize) -> u8 {
        unsafe {
            core::ptr::read_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *const u8,
            )
        }
    }
    fn read_u16(&self, address: usize) -> u16 {
        unsafe {
            core::ptr::read_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *const u16,
            )
        }
    }
    fn read_u32(&self, address: usize) -> u32 {
        unsafe {
            core::ptr::read_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *const u32,
            )
        }
    }
    fn read_u64(&self, address: usize) -> u64 {
        unsafe {
            core::ptr::read_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *const u64,
            )
        }
    }

    fn write_u8(&self, address: usize, value: u8) {
        unsafe {
            core::ptr::write_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *mut u8,
                value,
            )
        }
    }
    fn write_u16(&self, address: usize, value: u16) {
        unsafe {
            core::ptr::write_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *mut u16,
                value,
            )
        }
    }
    fn write_u32(&self, address: usize, value: u32) {
        unsafe {
            core::ptr::write_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *mut u32,
                value,
            )
        }
    }
    fn write_u64(&self, address: usize, value: u64) {
        unsafe {
            core::ptr::write_volatile(
                (self.mapper.phys_offset().as_u64() as usize + address) as *mut u64,
                value,
            )
        }
    }

    // IO端口访问（x86）
    fn read_io_u8(&self, port: u16) -> u8 {
        unsafe {
            let value: u8;
            core::arch::asm!("in al, dx", in("dx") port, out("al") value);
            value
        }
    }

    fn read_io_u16(&self, port: u16) -> u16 {
        unsafe {
            let value: u16;
            core::arch::asm!("in ax, dx", in("dx") port, out("ax") value);
            value
        }
    }

    fn read_io_u32(&self, port: u16) -> u32 {
        unsafe {
            let value: u32;
            core::arch::asm!("in eax, dx", in("dx") port, out("eax") value);
            value
        }
    }

    fn write_io_u8(&self, port: u16, value: u8) {
        unsafe { core::arch::asm!("out dx, al", in("dx") port, in("al") value) }
    }

    fn write_io_u16(&self, port: u16, value: u16) {
        unsafe { core::arch::asm!("out dx, ax", in("dx") port, in("ax") value) }
    }

    fn write_io_u32(&self, port: u16, value: u32) {
        unsafe { core::arch::asm!("out dx, eax", in("dx") port, in("eax") value) }
    }

    // 下面几个可以暂时不实现
    fn read_pci_u8(&self, _address: PciAddress, _offset: u16) -> u8 {
        0
    }
    fn read_pci_u16(&self, _address: PciAddress, _offset: u16) -> u16 {
        0
    }
    fn read_pci_u32(&self, _address: PciAddress, _offset: u16) -> u32 {
        0
    }

    fn write_pci_u8(&self, _address: PciAddress, _offset: u16, _value: u8) {}
    fn write_pci_u16(&self, _address: PciAddress, _offset: u16, _value: u16) {}
    fn write_pci_u32(&self, _address: PciAddress, _offset: u16, _value: u32) {}

    fn nanos_since_boot(&self) -> u64 {
        0 // 以后可用HPET或TSC实现
    }

    fn stall(&self, _microseconds: u64) {}
    fn sleep(&self, _milliseconds: u64) {}

    fn create_mutex(&self) -> Handle {
        todo!()
    }

    fn acquire(&self, mutex: Handle, timeout: u16) -> Result<(), AmlError> {
        todo!()
    }

    fn release(&self, mutex: Handle) {
        todo!()
    }
}
