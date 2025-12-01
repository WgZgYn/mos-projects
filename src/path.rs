use crate::command::Profile;
use std::path::{Path, PathBuf};

pub const PROJECT_PATH: &str = r#"F:\RustroverProjects\mos-projects"#;
pub const KERNEL_PATH: &str = r#"F:\RustroverProjects\mos-projects\mos"#;
pub const KERNEL_NAME: &str = "mos";
pub const USER_SPACE_PATH: &str = r#"F:\RustroverProjects\mos-projects\user_space"#;
pub const USER_SPACE_BINS_DIR: &str = r#"F:\RustroverProjects\mos-projects\user_space\src\bin"#;
pub const TARGET_PATH: &str = r#"F:\RustroverProjects\mos-projects\target"#;
pub const ARCH: &str = "x86_64-unknown-none";
pub const RAMFS_OUT_PATH: &str = r#"F:\RustroverProjects\mos-projects\ramfs.bin"#;
pub const BOOT_IMAGES_DIR: &str = r#"F:\RustroverProjects\mos-projects\bootimages"#;
pub fn elf_target_path<P: AsRef<Path>>(profile: Profile, name: P) -> PathBuf {
    Path::new(TARGET_PATH)
        .join(ARCH)
        .join(profile.as_str())
        .join(name)
}
pub fn rs_file_name(name: &str) -> &str {
    name.trim_end_matches(".rs")
}
