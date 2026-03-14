fn main() {
    println!("cargo:rustc-link-arg=-Ttest_dyn/user_dyn_linker.ld");
}