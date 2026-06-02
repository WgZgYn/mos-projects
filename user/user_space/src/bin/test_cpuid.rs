#![no_std]
#![no_main]

use user_lib::{getcpu, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let cpuid = getcpu();
    println!("cpuid: {}", cpuid);
    0
}
