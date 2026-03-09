#![no_std]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod syscall;
pub mod memory;
pub mod fs;
mod code;
pub mod io;
