#![no_std]
#![feature(abi_x86_interrupt)]
mod idt;

extern crate alloc;
pub use idt::install_handlers;