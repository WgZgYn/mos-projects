// src/main.rs

fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    let uefi = cfg!(feature = "uefi");
    println!("{uefi_path}, {bios_path}");
    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }
    cmd.arg("-device").arg("qemu-xhci");
    cmd.arg("-device").arg("usb-kbd");
    cmd.arg("-serial").arg("stdio");
    cmd.arg("-cpu").arg("qemu64,+rdrand");
    cmd.arg("-m").arg("4096");

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
