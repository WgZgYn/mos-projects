#![no_std]
extern crate alloc;

pub mod constant;
mod frame;
pub mod fs;
mod fs_manager;
mod index;
mod inode;
pub mod prelude;
mod utils;
pub mod vfs;

pub use fs_manager::RamFsManager;
