use crate::command::{BootMode, Build, BuildTarget, Cli, Commands, Profile, Run};
use crate::path::{
    BOOT_IMAGES_OUT_DIR, KERNEL_NAME, KERNEL_PATH, QEMU_X86_64_PATH, RAMFS_OUT_PATH,
    USER_SPACE_BINS_DIR, USER_SPACE_PATH, kernel_elf_path, kernel_target_path, rs_file_name,
    user_space_target_path,
};
use bootloader::BootConfig;
use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
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
    let mut cmd = std::process::Command::new(QEMU_X86_64_PATH);
    match boot_mode.unwrap_or(BootMode::Uefi) {
        BootMode::Uefi => {
            // this will download latest built from GitHub
            let prebuilt =
                Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to fetch prebuilt");

            let code = prebuilt.get_file(Arch::X64, FileType::Code);
            let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

            cmd.arg("-drive")
                .arg(format!("format=raw,file={}/uefi.img", BOOT_IMAGES_OUT_DIR));

            cmd.arg("-drive").arg(format!(
                "if=pflash,format=raw,unit=0,file={},readonly=on",
                code.display()
            ));

            // copy vars and enable rw instead of snapshot if you want to store data (e.g. enroll secure boot keys)
            cmd.arg("-drive").arg(format!(
                "if=pflash,format=raw,unit=1,file={},snapshot=on",
                vars.display()
            ))
        }
        BootMode::Bios => cmd
            .arg("-drive")
            .arg(format!("format=raw,file={}/bios.img", BOOT_IMAGES_OUT_DIR)),
    };
    cmd.arg("-serial").arg("mon:stdio");
    cmd.arg("-cpu").arg("qemu64,+rdrand");
    cmd.arg("-m").arg("4G");

    cmd.arg("-device").arg("virtio-gpu-pci");
    // cmd.arg("-nographic");
    // cmd.arg("-vnc").arg(":0");
    cmd.arg("-audiodev").arg("dsound,id=snd0");
    cmd.arg("-machine").arg("pcspk-audiodev=snd0");

    if run.map_ramfs_memory {
        cmd.arg("-device")
            .arg(format!("loader,file={},addr=0x50000000", RAMFS_OUT_PATH)); // This use pmemsave to save the memory
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
    fn build_kernel_elf(
        profile: Profile,
        boot_mode: Option<BootMode>,
    ) -> Result<(), Box<dyn Error>> {
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
        Ok(())
    }

    fn build_image(profile: Profile, boot_mode: Option<BootMode>) -> Result<(), Box<dyn Error>> {
        let mut boot_config = BootConfig::default();
        boot_config.log_level = bootloader_boot_config::LevelFilter::Off;
        boot_config.frame_buffer_logging = false;
        boot_config.serial_logging = true;
        boot_config.frame_buffer.minimum_framebuffer_width = Some(1000);
        boot_config.frame_buffer.minimum_framebuffer_height = Some(800);

        match boot_mode.unwrap_or(BootMode::Uefi) {
            BootMode::Uefi => {
                bootloader::UefiBoot::new(&kernel_elf_path(profile))
                    .set_ramdisk(Path::new(RAMFS_OUT_PATH))
                    .set_boot_config(&boot_config)
                    .create_disk_image(Path::new(BOOT_IMAGES_OUT_DIR).join("uefi.img").as_path())?;
            }
            BootMode::Bios => {
                bootloader::BiosBoot::new(kernel_target_path(profile, KERNEL_NAME).as_path())
                    .set_ramdisk(Path::new(RAMFS_OUT_PATH))
                    .set_boot_config(&boot_config)
                    .create_disk_image(Path::new(BOOT_IMAGES_OUT_DIR).join("bios.img").as_path())?;
            }
        }
        Ok(())
    }

    build_kernel_elf(profile, boot_mode)?;
    build_image(profile, boot_mode)?;

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
        .map(|e| e.file_name());

    let builder = RamfsBuilder::new()?;
    for name in bins {
        let name = name.into_string().map_err(|_| "Failed to parse elf name")?;
        let bin_name = rs_file_name(&name);
        let f = std::fs::File::open(user_space_target_path(profile, bin_name))?;
        builder.insert_file(bin_name, f)?;
    }
    builder.finish(RAMFS_OUT_PATH)?;
    Ok(())
}
