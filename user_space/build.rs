fn main() {
    println!("cargo:rerun-if-changed=user_space/user_linker.ld");
    println!("cargo:rustc-link-arg=-Tuser_space/user_linker.ld");
}