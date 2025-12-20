use kernel_utils::PageAligned;
use log::info;
use ramfs::RamFsManager;
use ramfs::prelude::SuperBlock;
use ramfs::vfs::RamFsSuperBlock;
use std::fs::File;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ops::Deref;

pub struct RamfsBuilder {
    _metadata: Box<PageAligned<[u8; 1024 * 1024 * 4]>>,
    fs: RamFsSuperBlock,
}

impl Deref for RamfsBuilder {
    type Target = RamFsSuperBlock;
    fn deref(&self) -> &Self::Target {
        &self.fs
    }
}

impl RamfsBuilder {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let metadata: Box<MaybeUninit<PageAligned<[u8; 1024 * 1024 * 4]>>> = Box::new_uninit();
        let mut metadata = unsafe {
            let mut data = metadata.assume_init();
            data.fill(0);
            data
        };

        let buf_ptr = metadata.as_mut_ptr();
        let buf_len = metadata.len();

        info!("Creating 4MiB Ramfs");
        unsafe {
            Ok(Self {
                _metadata: metadata,
                fs: RamFsSuperBlock::new(
                    RamFsManager::build(core::slice::from_raw_parts_mut(buf_ptr, buf_len))
                        .map_err(|_| "Ramfs build failed")?,
                ),
            })
        }
    }

    pub fn insert_file(&self, name: &str, mut f: File) -> Result<(), Box<dyn std::error::Error>> {
        info!("Inserting file {}", name);
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        let root = self.root();
        let node = root
            .create(name, vfs::FileType::Regular)
            .map_err(|_| "Ramfs create failed")?;

        node.write_at(0, buffer.as_slice())
            .map_err(|_| "Ramfs file write failed")?;
        Ok(())
    }

    pub fn finish(&self, ramfs_out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        info!("Finished building Ramfs");
        info!("Ramfs in {}", ramfs_out_path);
        let bs = &self._metadata.0;
        let mut f = File::create(ramfs_out_path).map_err(|_| "Failed to create ramfs file")?;
        f.write_all(bs).expect("should save ramfs ok");
        f.flush().map_err(|_| "Failed to flush ramfs file")?;
        Ok(())
    }
}
