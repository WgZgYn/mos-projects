use kernel_utils::PageAligned;
use ramfs::fs::FileType;
use ramfs::prelude::{FileSystem, Write as FWrite};
use ramfs::RamFsManager;
use std::ffi::OsString;
use std::fs::File;
use std::io::{Read, Write};
use std::mem::MaybeUninit;


const USER_SPACE_DIR: &str = r#"F:\RustroverProjects\mos-projects\user_space"#;
const BINS_DIR: &str = r#"F:\RustroverProjects\mos-projects\user_space\src\bin"#;
const TARGET_DIR: &str = r#"F:\RustroverProjects\mos-projects\target\x86_64-unknown-none\release"#;
const OUT_DIR: &str = r#"F:\RustroverProjects\mos-projects"#;


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
        Ok(Self {
            fs: RamFsManager::global().map_err(|e| e.as_str())?,
        })
    }

    pub fn insert_file(&self, name: &str, mut f: File) -> Result<(), Box<dyn std::error::Error>> {
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        let mut f = self
            .fs
            .create(&name, FileType::File)
            .map_err(|e| e.as_str())?;
        f.write(buffer.as_slice()).map_err(|e| e.as_str())?;
        Ok(())
    }

    pub fn finish(&self, ramfs_out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let gfs = self.fs.lock();
        let bs = unsafe { gfs.as_raw_bytes() };
        let mut f = File::create(ramfs_out_path).map_err(|_| "Failed to create ramfs file")?;
        f.write_all(bs).expect("should save ramfs ok");
        f.flush().map_err(|_| "Failed to flush ramfs file")?;
        Ok(())
    }
}

pub fn ramfs_build(elfs_path: Vec<(String, OsString)>) -> Result<(), Box<dyn std::error::Error>> {
    let buidler = RamfsBuilder::new()?;
    for (name, elf_path) in elfs_path {
        println!("Loading elf: {}, elf_path: {:?}", name, elf_path);
        buidler.insert_file(&name, File::open(elf_path)?)?;
    }
    buidler.finish("ramfs.bin")?;
    Ok(())
}
