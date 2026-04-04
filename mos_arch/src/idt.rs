use crate::gdt::DOUBLE_FAULT_IST_INDEX;
use mos_lib::sync::DummyLock;
use x86_64::PrivilegeLevel;
use x86_64::structures::idt::{
    DivergingHandlerFuncWithErrCode, HandlerFunc, HandlerFuncWithErrCode, InterruptDescriptorTable,
    PageFaultHandlerFunc,
};

static IDT: DummyLock<InterruptDescriptorTable> = DummyLock::new(InterruptDescriptorTable::new());

pub fn set_double_fault_handler(handler: DivergingHandlerFuncWithErrCode) {
    unsafe {
        IDT.access(|idt| {
            idt.double_fault
                .set_handler_fn(handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        });
    }
}

pub fn set_general_protection_fault_handler(handler: HandlerFuncWithErrCode) {
    unsafe {
        IDT.access(|idt| {
            idt.general_protection_fault.set_handler_fn(handler);
        })
    }
}

pub fn set_page_fault_handler(handler: PageFaultHandlerFunc) {
    unsafe {
        IDT.access(|idt| {
            idt.page_fault.set_handler_fn(handler);
        })
    }
}

pub fn set_breakpoint_handler(handler: HandlerFunc) {
    unsafe {
        IDT.access(|idt| {
            idt.breakpoint.set_handler_fn(handler);
        })
    }
}

pub fn set_general_handler(index: u8, handler: HandlerFunc) {
    unsafe {
        IDT.access(|idt| {
            idt[index].set_handler_fn(handler);
        })
    }
}

pub fn set_trap_handler(index: u8, handler: HandlerFunc) {
    unsafe {
        IDT.access(|idt| {
            idt[index]
                .set_handler_fn(handler)
                .set_privilege_level(PrivilegeLevel::Ring3);
        })
    }
}

pub fn load_idt() {
    unsafe {
        IDT.access(|idt| {
            idt.load();
        })
    }
}
