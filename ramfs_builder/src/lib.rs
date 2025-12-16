use kernel_utils::PageAligned;
use log::info;
use ramfs::RamFsManager;
use ramfs::fs::FileType;
use ramfs::prelude::{FileSystem, Write as FWrite};
use std::fs::File;
use std::io::{Read, Write};
use std::mem::MaybeUninit;

pub struct RamfsBuilder {
    fs: &'static RamFsManager<'static>,
}

impl RamfsBuilder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let metadata: Box<MaybeUninit<PageAligned<[u8; 1024 * 1024 * 4]>>> = Box::new_uninit();
        let mut metadata = unsafe { metadata.assume_init() };
        metadata.0.fill(0);
        let static_metadata = Box::leak(metadata);
        let gfs = RamFsManager::build(&mut static_metadata.0).map_err(|e| e.as_str())?;
        gfs.insert_to_global().expect("insert error");
        info!("Creating 4MiB Ramfs");
        Ok(Self {
            fs: RamFsManager::global().map_err(|e| e.as_str())?,
        })
    }

    pub fn insert_file(&self, name: &str, mut f: File) -> Result<(), Box<dyn std::error::Error>> {
        info!("Inserting file {}", name);
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        let mut f = self
            .fs
            .create(name, FileType::File)
            .map_err(|e| e.as_str())?;
        f.write(buffer.as_slice()).map_err(|e| e.as_str())?;
        Ok(())
    }

    pub fn finish(&self, ramfs_out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("Finished building Ramfs");
        info!("Ramfs in {}", ramfs_out_path);
        let gfs = self.fs.lock();
        let bs = unsafe { gfs.as_raw_bytes() };
        let mut f = File::create(ramfs_out_path).map_err(|_| "Failed to create ramfs file")?;
        f.write_all(bs).expect("should save ramfs ok");
        f.flush().map_err(|_| "Failed to flush ramfs file")?;
        Ok(())
    }
}
