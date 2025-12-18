#![no_std]
extern crate alloc;

pub mod constant;
pub mod file;
mod frame;
pub mod fs;
mod fs_manager;
mod index;
mod inode;
pub mod prelude;
mod utils;
pub mod vfs;

pub use fs_manager::RamFsManager;

static RAMFS_MANAGER: conquer_once::spin::OnceCell<RamFsManager<'static>> =
    conquer_once::spin::OnceCell::uninit();
