#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec;
use core::arch::asm;
use linked_list_allocator::{align_up, LockedHeap};
use user_space::println;

#[global_allocator]
static HEAP: LockedHeap = LockedHeap::empty();

fn get_brk() -> *const u8 {
    let brk: *const u8;
    unsafe {
        asm!("syscall", in("rax") 12, in("rdi") 0, lateout("rax") brk);
    }
    brk
}

fn set_brk(brk: *const u8) -> bool {
    let brk_page = align_up(brk.cast_mut(), 4096);
    let mut ok: isize = -1;
    unsafe {
        asm!("syscall", in("rax") 12, in("rdi") brk_page, lateout("rax") ok);
    }
    ok == 0
}

fn init_heap() {
    let old_brk = get_brk();
    unsafe {
        if !set_brk(old_brk.add(4096 * 4)) {
            panic!("failed to set heap brk");
        }
        let new_brk = get_brk();
        HEAP.lock()
            .init(old_brk.cast_mut(), new_brk.offset_from_unsigned(old_brk));
    }
}

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
