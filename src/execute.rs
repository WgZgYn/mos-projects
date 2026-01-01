use crate::command::{BootMode, Build, BuildTarget, Cli, Commands, Profile, Run};
use crate::path::{
    BOOT_IMAGES_DIR, KERNEL_NAME, KERNEL_PATH, RAMFS_OUT_PATH, USER_SPACE_BINS_DIR,
    USER_SPACE_PATH, elf_target_path, rs_file_name,
};
use ramfs_builder::RamfsBuilder;
use std::error::Error;
use std::path::Path;

pub fn parse_run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::Run(r) => run(r, cli.profile, cli.boot_mode),
        Commands::Build(b) => build(b, cli.profile, cli.boot_mode),
    }
}

pub fn run(run: Run, profile: Profile, boot_mode: Option<BootMode>) -> Result<(), Box<dyn Error>> {
    println!("Running on mode: {:?}:{:?}", profile, boot_mode);
    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    match boot_mode.unwrap_or(BootMode::Uefi) {
        BootMode::Uefi => cmd
            .arg("-bios")
            .arg(ovmf_prebuilt::ovmf_pure_efi())
            .arg("-drive")
            .arg(format!("format=raw,file={}/uefi.img", BOOT_IMAGES_DIR,)),
        BootMode::Bios => cmd
            .arg("-drive")
            .arg(format!("format=raw,file={}/bios.img", BOOT_IMAGES_DIR,)),
    };
    cmd.arg("-serial").arg("mon:stdio");
    cmd.arg("-cpu").arg("qemu64,+rdrand");
    cmd.arg("-m").arg("4G");

    cmd.arg("-device").arg("virtio-gpu-pci");
    // cmd.arg("-nographic");
    // cmd.arg("-vnc").arg(":0");

    if run.map_ramfs_memory {
        cmd.arg("-device")
            .arg("loader,file=./ramfs.bin,addr=0x50000000"); // This use pmemsave to save the memory
    }
    cmd.status()?;
    Ok(())
}

pub fn build(
    build: Build,
    profile: Profile,
    boot_mode: Option<BootMode>,
) -> Result<(), Box<dyn Error>> {
    match build.target {
        BuildTarget::Kernel => build_kernel(profile, boot_mode),
        BuildTarget::Ramfs => build_ramfs(profile),
        BuildTarget::All => {
            build_ramfs(profile)?;
            build_kernel(profile, boot_mode)
        }
    }
}

pub fn build_kernel(profile: Profile, boot_mode: Option<BootMode>) -> Result<(), Box<dyn Error>> {
    let mut cmd = std::process::Command::new("cargo");
    cmd.current_dir(KERNEL_PATH)
        .args(["build", &format!("--{}", profile.as_str())]);

    match boot_mode {
        Some(BootMode::Uefi) => {
            cmd.arg("--no-default-features")
                .arg("--features")
                .arg("intr_apic");
        }
        Some(BootMode::Bios) => {
            cmd.arg("--no-default-features")
                .arg("--features")
                .arg("intr_legacy");
        }
        _ => {}
    };
    cmd.status()?;

    match boot_mode.unwrap_or(BootMode::Uefi) {
        BootMode::Uefi => {
            bootloader::UefiBoot::new(elf_target_path(profile, KERNEL_NAME).as_path())
                .set_ramdisk(Path::new(RAMFS_OUT_PATH))
                .create_disk_image(Path::new(BOOT_IMAGES_DIR).join("uefi.img").as_path())?;
        }
        BootMode::Bios => {
            bootloader::BiosBoot::new(elf_target_path(profile, KERNEL_NAME).as_path())
                .set_ramdisk(Path::new(RAMFS_OUT_PATH))
                .create_disk_image(Path::new(BOOT_IMAGES_DIR).join("bios.img").as_path())?;
        }
    }

    Ok(())
}
pub fn build_ramfs(profile: Profile) -> Result<(), Box<dyn Error>> {
    fn build_user_space_bins(profile: Profile) -> Result<(), Box<dyn Error>> {
        std::process::Command::new("cargo")
            .current_dir(USER_SPACE_PATH)
            .args(["build", &format!("--{}", profile.as_str())])
            .status()?;
        Ok(())
    }
    build_user_space_bins(profile)?;
    let bins = std::fs::read_dir(USER_SPACE_BINS_DIR)?
        .flatten()
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.file_name())
        .collect::<Vec<_>>();
    let builder = RamfsBuilder::new()?;
    for name in bins {
        let name = name.into_string().map_err(|_| "Failed to parse elf name")?;
        let bin_name = rs_file_name(&name);
        let f = std::fs::File::open(elf_target_path(profile, bin_name))?;
        builder.insert_file(bin_name, f)?;
    }
    builder.finish(RAMFS_OUT_PATH)?;
    Ok(())
}
