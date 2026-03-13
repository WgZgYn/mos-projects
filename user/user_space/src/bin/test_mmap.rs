#![no_std]
#![no_main]

use mos_lib::memory::{MemoryMapFlags, MemoryProtFlags};
use user_space::{exit, init_heap, mmap, open, println, write};
use vfs::OpenFlags;

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();

    let f = open(
        "mmap.txt",
        OpenFlags::READ_WRITE | OpenFlags::CREATE | OpenFlags::APPEND,
    )
    .expect("should open or create mmap.txt");
    write(f, b"hello world!\n");

    let fmap = mmap(
        0x4234_0000,
        4096,
        MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE,
        Some(f),
        0,
    );

    println!("fmap: {:X?}", fmap);
    if fmap == u64::MAX {
        exit(-1);
    }

    let virt_addr = fmap as *const u8;
    let slice = unsafe { core::slice::from_raw_parts_mut(virt_addr.cast_mut(), 4096) };

    println!("fmap: {:?}", &slice[..10]);

    let ret = mmap(
        0,
        4096,
        MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::ANONYMOUS,
        None,
        0,
    );

    println!("ret: {:X?}", ret);

    if ret == u64::MAX {
        println!("mmap failed");
        exit(-1);
    }

    let virt_addr = ret as *const u8;
    let slice = unsafe { core::slice::from_raw_parts_mut(virt_addr.cast_mut(), 4096) };
    slice[0] = 123;
    slice[1] = 124;
    println!("mmap data: {:?}", &slice[..5]);
    0
}
