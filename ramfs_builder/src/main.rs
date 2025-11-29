use std::io::Write;
use log::{info, trace, LevelFilter};
use ramfs::fs::{Error, FileType};
use ramfs::prelude::{FileSystem, OpenFlag, Read, Write as FWrite};
use ramfs::RamFsManager;
use std::path::Path;


fn main() -> Result<(), &'static str> {
    env_logger::builder()
        .filter_level(LevelFilter::Trace)
        .init();

    let target = Path::new("x86_64-unknown-none");
    let profile = "release";
    let user_space_path = Path::new(r#"F:\RustroverProjects\mos-projects\user_space"#);
    let target_path = Path::new(r#"F:\RustroverProjects\mos-projects\target"#)
        .join(target)
        .join(profile);

    let gfs = RamFsManager::global().map_err(|e| e.as_str())?;

    gfs.lock().ls().iter().for_each(|item| {
        println!("File name: {}", item.name());
        println!("File type: {:#?}", item.file_type());
        println!();
    });

    println!("current dir: {:?}", std::env::current_dir());
    let user_crates = std::fs::read_dir(user_space_path)
        .map_err(|_| "Failed to read user space directory")?
        .flatten()
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    println!(
        "manifest files: {:?}, total: {} crates",
        user_crates,
        user_crates.len()
    );
    let mut success_elf = Vec::new();
    for dir_path in user_crates {
        if let Ok(dir_content) = std::fs::read_dir(&dir_path) {
            // skip the dir not contains the Cargo.toml.
            if dir_content
                .flatten()
                .find(|s| s.file_name() == "Cargo.toml")
                .is_none()
            {
                continue;
            }

            // spawn the cargo process
            let res = std::process::Command::new("cargo")
                .current_dir(&dir_path)
                .arg("build")
                .arg("--release")
                .status();

            if let Err(err) = res {
                println!("Failed to run cargo build: {}", err);
                continue;
            }

            println!("cargo build {:?} success", dir_path.file_name());
            success_elf.push(dir_path.file_name().ok_or("filename empty")?.to_os_string());
        }
    }

    let file_set: Vec<_> = std::fs::read_dir(target_path)
        .map_err(|_| "Failed to read target directory")?
        .flatten()
        .filter(|entry| entry.path().is_file())
        .map(|entry| entry.path())
        .collect();

    for elf in success_elf {
        if let Some(p) = file_set
            .iter()
            .find(|p| p.file_name() == Some(elf.as_ref()))
        {
            println!("File name: {:#?}", p);
            let file_bs = std::fs::read(p).map_err(|_| "Failed to read file")?;
            let name = elf.to_str().ok_or("can't trans OsStr to str")?;
            let f = gfs.open(name, OpenFlag::WriteOnly);
            let f = match f {
                Ok(f) if f.file_type() == Ok(FileType::File) => Some(f),
                Err(Error::NoSuchFile) => gfs.create(name, FileType::File).ok(),
                _ => continue,
            };
            if let Some(mut f) = f {
                println!("Write {name} to Ramfs");
                f.write(file_bs.as_slice())
                    .map_err(|_| "Failed to write file")?;

                let mut fr = gfs
                    .open(name, OpenFlag::ReadOnly)
                    .map_err(|e| e.as_str())?;
                let mut buffer = [0; 16];
                let n = fr.read(&mut buffer).map_err(|e| e.as_str())?;
                println!("Read {n} from ramfs's {name}: {:?}", &buffer[..n]);
            }
        }
    }

    gfs.lock().ls().iter().for_each(|item| {
        println!("File name: {}", item.name());
        println!("File type: {:#?}", item.file_type());
        println!();
    });

    let g = gfs.lock();
    let bs = unsafe { g.as_raw_bytes() };

    let file_name = format!(
        "../ramfs-{}.bin",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("Time went backwards")
            .as_secs()
    );
    let mut f = std::fs::File::create(&file_name).map_err(|_| "Failed to create ramfs file")?;
    f.write_all(bs).expect("should save ramfs ok");
    println!("Successfully wrote to {}", file_name);
    f.flush().map_err(|_| "Failed to flush ramfs file")?;

    trace!("fs_metadata_start: {:x?}", gfs.lock().fs_start());
    info!("magic: {:?}", gfs.lock().magic());

    let f = gfs.create("foo", FileType::File).map_err(|e| e.as_str())?;

    let fname = f.file_name().map_err(|e| e.as_str())?;
    trace!("name: {:?}", fname);

    gfs.lock()
        .ls()
        .iter()
        .for_each(|e| trace!("{:?}", e.name()));

    Ok(())
}
