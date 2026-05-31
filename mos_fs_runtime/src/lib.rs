#![no_std]

extern crate alloc;

use alloc::sync::Arc;
use log::info;
use mem_fs::get_memfs_root;
use mos_devfs::DEV_DIR_INODE;
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use mos_vfs::vnode::{InodeExt, VfsInode};
use mos_vfs::{OpenResult, mount, register_root_inode, vfs_open, vfs_root_dentry};
use ramfs::RamFsManager;
use ramfs::vfs::RamFsSuperBlock;
use vfs_common::*;
use vfs_traits::*;
use crate::pipe::make_fifo_inode;

pub mod pipe;

pub fn vfs_init(ramfs_addr: u64, ramfs_len: u64) {
    load_ramfs(ramfs_addr, ramfs_len);
    init_mount();
}

pub fn load_ramfs(addr: u64, len: u64) {
    info!("[VFS] load ramfs");
    let fs = unsafe { RamFsManager::load(core::slice::from_raw_parts_mut(addr as _, len as _)) }
        .expect("RamFsManager::load()");
    let super_block = RamFsSuperBlock::new(fs);
    register_root_inode(super_block.root());
}

pub fn init_mount() {
    info!("[VFS] mount devfs");
    mount_dev_fs();
    info!("[VFS] mount memfs");
    mount_mem_fs();
}

pub fn mount_mem_fs() {
    let memfs_root = get_memfs_root();
    let memfs_root = Dentry::create_root(Arc::new(VfsInode::new(memfs_root)));

    // TODO: replace with mkdir
    let mem_dir_file = open(
        "/mem",
        OpenFlags::READ | OpenFlags::DIRECTORY | OpenFlags::CREATE,
    )
        .expect("failed to open /mem");

    let memfs_dentry = mem_dir_file.dentry().clone();
    mount(memfs_root, memfs_dentry).expect("failed to mount memfs at /mem");
}

pub fn mount_dev_fs() {
    // init devfs dentry
    let devfs_root = Dentry::create_root(Arc::new(VfsInode::new(DEV_DIR_INODE.clone())));

    // TODO: replace with mkdir
    // create ramfs /dev dir
    let dev_dir_file = open(
        "/dev",
        OpenFlags::READ | OpenFlags::DIRECTORY | OpenFlags::CREATE,
    )
        .expect("open failed");
    let dev_dentry = dev_dir_file.dentry().clone();

    // mount devfs at /dev
    mount(dev_dentry, devfs_root).expect("mount failed");
}

pub fn open(abs_path: &str, flags: OpenFlags) -> VfsResult<File> {
    open_with_dentry(vfs_root_dentry(), vfs_root_dentry(), abs_path, flags)
}

pub fn open_with_dentry(
    root: Arc<Dentry>,
    curr: Arc<Dentry>,
    path: &str,
    flags: OpenFlags,
) -> VfsResult<File> {
    let ret = vfs_open(root, curr, path, flags)?;
    match ret {
        OpenResult::File(f) => Ok(f),
        OpenResult::FirstOpenLogicFile(d) => match d.file_type()? {
            FileType::Fifo => d.open_with(InodeExt::Fifo(make_fifo_inode())),
            _ => Err(VfsError::NotSupported),
        },
    }
}
