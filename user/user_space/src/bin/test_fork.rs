#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use user_lib::{fork, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("===== cow_stress_test start =====");

    const PAGE: usize = 4096;
    const NPAGES: usize = 4;

    // 在堆上分配多页连续内存
    let mut vec = Vec::with_capacity(PAGE * NPAGES);
    vec.resize(PAGE * NPAGES, 0u8);

    // 初始化每一页
    for i in 0..NPAGES {
        vec[i * PAGE] = i as u8;
    }

    println!("parent: initial write done");

    let pid1 = fork();

    if pid1 == 0 {
        // 子进程 1
        println!("[L1 CHILD] writing all pages");

        for i in 0..NPAGES {
            vec[i * PAGE] = 100 + i as u8;
        }

        // 再写一次（测试二次写不会再次 #PF）
        for i in 0..NPAGES {
            vec[i * PAGE] += 1;
        }

        println!("[L1 CHILD] done writing");

        let pid2 = fork();
        if pid2 == 0 {
            println!("[L2 CHILD] deep write test");

            // 深层子进程再写
            for i in 0..NPAGES {
                vec[i * PAGE] += 1;
            }

            println!("[L2 CHILD] finished");
        } else {
            println!("[L1 CHILD] forked L2 child {}", pid2);
        }
    } else {
        // 父进程
        println!("[PARENT] first fork = {}", pid1);

        // 父进程写一部分页（与子交错）
        vec[0] = 42;
        vec[PAGE] = 43;

        println!("[PARENT] partial write done");

        let pid3 = fork();
        if pid3 == 0 {
            println!("[L1 CHILD2] writing last pages");

            vec[2 * PAGE] = 88;
            vec[3 * PAGE] = 99;

            println!("[L1 CHILD2] done");
        } else {
            println!("[PARENT] second fork = {}", pid3);
        }
    }

    // 最终验证数据一致性
    println!(
        "process done, values: {}, {}, {}, {}",
        vec[0],
        vec[PAGE],
        vec[2 * PAGE],
        vec[3 * PAGE],
    );

    println!("===== cow_stress_test end =====");
    0
}
