#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec;
use user_space::{init_heap, println};

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("--- test_heap --- start");
    init_heap();
    let mut array = vec![1, 2, 3];
    array.push(4);
    println!("{:?}", array);
    println!("--- test_heap --- end");
    0
}
