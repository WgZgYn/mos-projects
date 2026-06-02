use crate::command::{BootMode, Build, BuildTarget, Cli, Commands, Profile, Run};
use crate::path::{
    KERNEL_NAME, QEMU_X86_64_PATH, RAMFS_ADDR, RAMFS_FILE_OUT_PATH, RAMFS_ROOT_DIR_PATH,
    image_output_naming, kernel_elf_output_naming,
};
use bootloader::BootConfig;
use log::{error, info};
use ovmf_prebuilt::{Arch, FileType, Prebuilt, Source};
use serde_json::Value;
use std::error::Error;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Command, Stdio};

trait LogResult {
    fn log_err(self, msg: &str) -> Self;
}

impl<T, E> LogResult for Result<T, E> {
    fn log_err(self, msg: &str) -> Self {
        if self.is_err() {
            error!("{}", msg);
        }
        self
    }
}

pub fn parse_run(cli: Cli) -> Result<(), Box<dyn Error>> {
    match cli.command {
        Commands::Run(r) => run(r, cli.profile, cli.boot_mode).log_err("while running"),
        Commands::Build(b) => build(b, cli.profile, cli.boot_mode).log_err("while building"),
    }
}

pub fn run(run: Run, profile: Profile, boot_mode: Option<BootMode>) -> Result<(), Box<dyn Error>> {
    println!("Running on mode: {:?}:{:?}", profile, boot_mode);
    let mut cmd = Command::new(QEMU_X86_64_PATH);
    let boot_mode = boot_mode.unwrap_or(BootMode::Uefi);
    let image_path = image_output_naming(KERNEL_NAME, boot_mode);
    match boot_mode {
        BootMode::Uefi => {
            // this will download latest built from GitHub
            let prebuilt =
                Prebuilt::fetch(Source::LATEST, "target/ovmf").expect("failed to fetch prebuilt");

            let code = prebuilt.get_file(Arch::X64, FileType::Code);
            let vars = prebuilt.get_file(Arch::X64, FileType::Vars);

            cmd.arg("-drive")
                .arg(format!("format=raw,file={}", image_path));

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
            .arg(format!("format=raw,file={}", image_path)),
    };
    cmd.arg("-serial").arg("mon:stdio");
    cmd.arg("-cpu").arg("qemu64,+rdrand");
    cmd.arg("-m").arg("4G");
    cmd.arg("-smp").arg("4");
    cmd.arg("-device").arg("virtio-gpu-pci");
    // cmd.arg("-nographic");
    // cmd.arg("-vnc").arg(":0");
    cmd.arg("-audiodev").arg("dsound,id=snd0");
    cmd.arg("-machine").arg("pcspk-audiodev=snd0");
    // cmd.arg("-d").arg("int,guest_errors,cpu_reset").arg("-D").arg("qemu.log");
    // cmd.arg("-no-reboot").arg("-no-shutdown");
    // cmd.arg("-netdev").arg("user,id=net0");
    // cmd.arg("-device").arg("e1000,netdev=net0,mac=52:54:00:12:34:56");

    if run.map_ramfs_memory {
        cmd.arg("-device").arg(format!(
            "loader,file={},addr={}",
            RAMFS_FILE_OUT_PATH, RAMFS_ADDR
        )); // This use pmemsave to save the memory
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
        BuildTarget::User => build_user(RAMFS_ROOT_DIR_PATH).log_err("while building user"),
        BuildTarget::Kernel => {
            build_image(KERNEL_NAME, profile, boot_mode).log_err("while building kernel")
        }
        BuildTarget::Ramfs => {
            build_ramfs(RAMFS_ROOT_DIR_PATH, RAMFS_FILE_OUT_PATH).log_err("while building ramfs")
        }
        BuildTarget::All => {
            build_user(RAMFS_ROOT_DIR_PATH).log_err("while building user")?;
            build_ramfs(RAMFS_ROOT_DIR_PATH, RAMFS_FILE_OUT_PATH)
                .log_err("while building ramfs")?;
            build_image(KERNEL_NAME, profile, boot_mode).log_err("while building image")
        }
    }
}

pub fn build_user(output_dir: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    user::build_user_space(output_dir)
}

pub fn build_image(
    kernel_name: &str,
    profile: Profile,
    boot_mode: Option<BootMode>,
) -> Result<(), Box<dyn Error>> {
    fn build_kernel_elf(
        kernel_name: &str,
        profile: Profile,
        boot_mode: Option<BootMode>,
    ) -> Result<String, Box<dyn Error>> {
        info!("Building kernel");
        let mut cmd = Command::new("cargo");
        cmd.current_dir(kernel_name).arg("build");
        if let Profile::Release = profile {
            cmd.arg("--release");
        }
        cmd.arg("--no-default-features").arg("--features");
        let boot_mode = boot_mode.unwrap_or(BootMode::Uefi);

        match boot_mode {
            BootMode::Bios => cmd.arg("intr_legacy"),
            BootMode::Uefi => cmd.arg("intr_apic"),
        };

        cmd.arg("--message-format=json-render-diagnostics")
            .stderr(Stdio::inherit())
            .stdout(Stdio::piped());
        let mut child = cmd.spawn()?;
        let output = child.stdout.take().ok_or("child has stdout")?;
        let reader = BufReader::new(output);
        let mut artifacts = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                if v["reason"] == "compiler-artifact" && v["target"]["name"] == kernel_name {
                    if let Some(exec) = v["executable"].as_str() {
                        artifacts.push(exec.to_string());
                    }
                }
            }
        }
        child.wait()?;
        if artifacts.len() != 1 {
            dbg!(&artifacts);
            return Err("failed to build kernel".into());
        }

        let dst = kernel_elf_output_naming(kernel_name, boot_mode);

        std::fs::copy(artifacts[0].as_str(), &dst)?;
        info!("Kernel ELF successfully built");
        Ok(dst)
    }

    fn build_image_inner(
        kernel_name: &str,
        kernel_path: impl AsRef<Path>,
        boot_mode: Option<BootMode>,
    ) -> Result<(), Box<dyn Error>> {
        let mut boot_config = BootConfig::default();
        boot_config.log_level = bootloader_boot_config::LevelFilter::Off;
        boot_config.frame_buffer_logging = false;
        boot_config.serial_logging = true;
        boot_config.frame_buffer.minimum_framebuffer_width = Some(2560);
        boot_config.frame_buffer.minimum_framebuffer_height = Some(1600);

        let boot_mode = boot_mode.unwrap_or(BootMode::Uefi);
        let image_out = image_output_naming(kernel_name, boot_mode);

        match boot_mode {
            BootMode::Uefi => {
                bootloader::UefiBoot::new(kernel_path.as_ref())
                    .set_ramdisk(Path::new(RAMFS_FILE_OUT_PATH))
                    .set_boot_config(&boot_config)
                    .create_disk_image(image_out.as_ref())?;
            }
            BootMode::Bios => {
                bootloader::BiosBoot::new(kernel_path.as_ref())
                    .set_ramdisk(Path::new(RAMFS_FILE_OUT_PATH))
                    .set_boot_config(&boot_config)
                    .create_disk_image(image_out.as_ref())?;
            }
        }
        Ok(())
    }

    let kernel_path = build_kernel_elf(kernel_name, profile, boot_mode)?;
    build_image_inner(kernel_name, kernel_path, boot_mode)?;

    Ok(())
}

pub fn build_ramfs(root: impl AsRef<Path>, out: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
    ramfs_builder::build_ramfs(root, out)?;
    Ok(())
}
