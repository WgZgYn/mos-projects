#![no_std]
#![no_main]

use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{create_thread, exit, mmap, println};

fn child_thread() -> ! {
    println!("[USER] child_thread println");
    exit(0)
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[USER] test_thread_sys");

    let start = mmap(
        0,
        1024 * 8,
        MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS,
        None,
        0,
    );
    if start == u64::MAX {
        println!("[USER] test_thread_sys failed to mmap stack space");
        return -1;
    }

    create_thread(child_thread as _, (start + 1024 * 8) as _);
    println!("[USER] main thread exit");
    0
}
