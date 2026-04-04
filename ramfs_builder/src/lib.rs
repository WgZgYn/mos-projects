use log::info;
use mos_lib::align::PageAligned;
use ramfs::RamFsManager;
use ramfs::vfs::RamFsSuperBlock;
use std::fs::File;
use std::io::{Read, Write};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
use vfs_common::FileType;
use vfs_traits::{Inode, SuperBlock};

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
            .as_dir_ops()
            .map_err(|e| e.as_str())?
            .create(name, FileType::Regular)
            .map_err(|_| "Ramfs create failed")?;

        node.as_file_ops()
            .map_err(|e| e.as_str())?
            .write_at(0, buffer.as_slice())
            .map_err(|_| "Ramfs file write failed")?;
        Ok(())
    }

    pub fn finish(
        &self,
        ramfs_out_path: impl AsRef<Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        info!("Finished building Ramfs");
        info!("Ramfs in {}", ramfs_out_path.as_ref().display());
        let bs = &self._metadata.0;
        let mut f = File::create(ramfs_out_path).map_err(|_| "Failed to create ramfs file")?;
        f.write_all(bs).expect("should save ramfs ok");
        f.flush().map_err(|_| "Failed to flush ramfs file")?;
        Ok(())
    }
}

pub fn build_ramfs(
    ramfsroot: impl AsRef<Path>,
    ramfsout: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let builder = RamfsBuilder::new()?;
    let fs_root = builder.root();
    build_helper(fs_root, ramfsroot)?;
    builder.finish(ramfsout.as_ref())
}

fn build_helper(
    fs_dir: Arc<dyn Inode>,
    os_path: impl AsRef<Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    let os_path = os_path.as_ref();
    let dir = os_path.read_dir()?;
    for entry in dir {
        let entry = entry?;
        let name = entry.file_name();
        let ft = entry.file_type();
        match (name.to_str(), ft) {
            (Some(name), Ok(ft)) if ft.is_file() => {
                let mut os_file = File::open(entry.path())?;
                let fs_file = fs_dir
                    .clone()
                    .as_dir_ops()
                    .map_err(|e| e.as_str())?
                    .create(name, FileType::Regular)
                    .map_err(|e| e.as_str())?;
                let mut buffer = Vec::new();
                os_file.read_to_end(&mut buffer)?;
                fs_file
                    .as_file_ops()
                    .map_err(|e| e.as_str())?
                    .write_at(0, &buffer)
                    .map_err(|e| e.as_str())?;
                info!("insert file {}:{:?} success", name, FileType::Regular);
            }
            (Some(name), Ok(ft)) if ft.is_dir() => {
                let next_os_dir = os_path.join(name);
                let fs_dir = fs_dir
                    .clone()
                    .as_dir_ops()
                    .map_err(|e| e.as_str())?
                    .create(name, FileType::Directory)
                    .map_err(|e| e.as_str())?;
                info!("insert file {}:{:?} success", name, FileType::Directory);
                build_helper(fs_dir, next_os_dir)?;
            }
            (Some(name), ft) => {
                info!(
                    "insert file {} failed with unsupported file type {:?}",
                    name, ft
                );
            }
            _ => {}
        }
    }
    Ok(())
}
