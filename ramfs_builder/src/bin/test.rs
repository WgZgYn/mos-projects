use kernel_utils::PageAligned;
use log::{LevelFilter, info};
use ramfs::RamFsManager;
use ramfs::vfs::RamFsSuperBlock;
use std::mem::MaybeUninit;
use vfs::{FileType, SuperBlock};

fn main() {
    env_logger::builder().filter_level(LevelFilter::Info).init();
    info!("starting up");
    let metadata: Box<MaybeUninit<PageAligned<[u8; 1024 * 1024 * 4]>>> = Box::new_uninit();
    let mut metadata = unsafe { metadata.assume_init() };
    let f = std::fs::read(r"../ramfs.bin").unwrap();
    metadata.0.copy_from_slice(&f);
    let static_metadata = Box::leak(metadata);
    let fs = RamFsManager::load(&mut static_metadata.0)
        .map_err(|e| e.as_str())
        .unwrap();
    let block = RamFsSuperBlock::new(fs);
    info!("Loaded");

    let root = block.root();
    for i in 0.. {
        let Ok(Some(d)) = root.readdir(i) else {
            break;
        };
        println!("name: {}, type: {:?}", d.name(), d.file_type());
    }

    let f = root
        .create("hello.txt", FileType::Regular)
        .expect("failed to create file");

    f.write_at(0, b"Hello, world!")
        .expect("failed to write file");
    drop(f);

    let f = root.lookup("hello.txt").expect("failed to lookup");
    let mut buf = [0; 32];
    let n = f.read_at(0, &mut buf).expect("failed to read");
    println!("{:?}", core::str::from_utf8(&buf[..n]));
    for i in 0.. {
        let Ok(Some(d)) = root.readdir(i) else {
            break;
        };
        println!("name: {}, type: {:?}", d.name(), d.file_type());
    }
}
