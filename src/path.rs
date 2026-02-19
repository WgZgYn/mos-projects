use crate::command::Profile;
use std::path::{Path, PathBuf};

pub const QEMU_X86_64_PATH: &str = "qemu-system-x86_64";

pub const KERNEL_NAME: &str = "mos";
pub const KERNEL_PATH: &str = r#"mos"#;
pub const USER_SPACE_PATH: &str = r#"user_space"#;
pub const USER_SPACE_BINS_DIR: &str = r#"user_space/src/bin"#;
pub const TARGET_PATH: &str = r#"target"#;
pub const RAMFS_OUT_PATH: &str = r#"build/ramfsout/ramfs.bin"#;
pub const BOOT_IMAGES_OUT_DIR: &str = r#"build/image"#;

pub fn kernel_target_path<P: AsRef<Path>>(profile: Profile, name: P) -> PathBuf {
    pub const KERNEL_ARCH: &str = "x86_64-unknown-none";
    Path::new(TARGET_PATH)
        .join(KERNEL_ARCH)
        .join(profile.as_str())
        .join(name)
}

pub fn kernel_elf_path(profile: Profile) -> PathBuf {
    kernel_target_path(profile, KERNEL_NAME)
}

pub fn user_space_target_path<P: AsRef<Path>>(profile: Profile, name: P) -> PathBuf {
    pub const USER_SPACE_ARCH: &str = "x86_64-mos";
    Path::new(TARGET_PATH)
        .join(USER_SPACE_ARCH)
        .join(profile.as_str())
        .join(name)
}

pub fn rs_file_name(name: &str) -> &str {
    name.trim_end_matches(".rs")
}
