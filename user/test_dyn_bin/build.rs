use std::path::Path;

fn main() {
    let current_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let path = current_dir
        .parent()
        .unwrap()
        .join("target")
        .join("x86_64-mos-dyn")
        .join("release");
    println!("cargo:rustc-link-search=native={}", path.display());
    println!("cargo:rustc-link-lib=test_dyn");

    println!("cargo:rustc-link-arg=-Ttest_dyn_bin/user_dyn_linker.ld");
}
