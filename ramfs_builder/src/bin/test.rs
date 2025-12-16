use kernel_utils::PageAligned;
use log::{LevelFilter, info};
use ramfs::RamFsManager;
use std::mem::MaybeUninit;

fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    info!("starting up");
    let metadata: Box<MaybeUninit<PageAligned<[u8; 1024 * 1024 * 4]>>> = Box::new_uninit();
    let mut metadata = unsafe { metadata.assume_init() };
    let f = std::fs::read(r"F:\RustroverProjects\mos-projects\ramfs.bin").unwrap();
    metadata.0.copy_from_slice(&f);
    let static_metadata = Box::leak(metadata);
    let gfs = RamFsManager::load(&mut static_metadata.0)
        .map_err(|e| e.as_str())
        .unwrap();
    gfs.insert_to_global().expect("insert error");

    info!("Loaded");

    let gfs = RamFsManager::global().unwrap();
    gfs.lock()
        .ls()
        .iter()
        .for_each(|i| println!("name: {}, type: {}", i.name(), i.file_type()));
}
