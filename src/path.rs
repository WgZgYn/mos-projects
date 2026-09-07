use crate::command::BootMode;

pub const QEMU_X86_64_PATH: &str = r"C:\Program Files\qemu\qemu-system-x86_64.exe";
pub const KERNEL_NAME: &str = "mos";
pub const RAMFS_ROOT_DIR_PATH: &str = r"build/ramfsroot";
pub const RAMFS_FILE_OUT_PATH: &str = r"build/ramfsout/ramfs.bin";
pub const RAMFS_ADDR: u64 = 0x50000000;

pub fn kernel_elf_output_naming(name: &str, boot_mode: BootMode) -> String {
    pub const KERNEL_ELF_OUT_DIR_PATH: &str = r"build/kernel";

    format!(
        "{}/{name}-{mode}",
        KERNEL_ELF_OUT_DIR_PATH,
        mode = boot_mode.as_str()
    )
}

pub fn image_output_naming(kernel_name: &str, boot_mode: BootMode) -> String {
    pub const IMAGE_OUT_DIR_PATH: &str = r"build/image";

    format!(
        "{}/{kernel_name}-{}.img",
        IMAGE_OUT_DIR_PATH,
        boot_mode.as_str()
    )
}
