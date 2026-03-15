#![no_std]
#![no_main]

use user_lib::{println, sleep};

fn test_sleep() {
    println!("--- call sleep ---");
    sleep(500);
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("----- test_sleep ----- start");
    test_sleep();
    println!("----- test_sleep ----- end");
    0
}
