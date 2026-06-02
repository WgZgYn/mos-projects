#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{create_thread, exit, getcpu, mmap, println, sem_create, sem_signal, sem_wait,
               sleep};

const WORKER_COUNT: u32 = 4;
const STACK_SIZE: u64 = 4096 * 4; // 16 KiB per thread

/// 两个信号量的 ID — 初始化后由 main 写入，worker 读取
static GO_SEM: AtomicU32 = AtomicU32::new(0); // main → workers
static DONE_SEM: AtomicU32 = AtomicU32::new(0); // workers → main

/// 共享计数器，验证所有 worker 都执行了
static COUNTER: AtomicU32 = AtomicU32::new(0);

fn worker(id: u32) -> ! {
    let go = GO_SEM.load(Ordering::Acquire) as u64;
    let done = DONE_SEM.load(Ordering::Acquire) as u64;
    let cpu = getcpu();

    println!("[W{}] cpu={} waiting for GO...", id, cpu);
    sem_wait(go);

    // 临界区: 递增共享计数器
    let c = COUNTER.fetch_add(1, Ordering::SeqCst) + 1;
    println!("[W{}] cpu={} acquired, counter = {}", id, cpu, c);

    // 通知 main 当前 worker 已完成
    sem_signal(done);
    exit(0)
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("=== Semaphore + Kernel Thread Test ===");
    println!("spawning {} worker threads...", WORKER_COUNT);

    // 创建两个信号量 (初始计数 = 0, 用作同步屏障)
    let go_sem = sem_create(0);
    let done_sem = sem_create(0);
    GO_SEM.store(go_sem as u32, Ordering::Release);
    DONE_SEM.store(done_sem as u32, Ordering::Release);
    println!("GO sem={} DONE sem={}", go_sem, done_sem);

    // 为每个 worker 分配栈空间并创建线程
    for i in 0..WORKER_COUNT {
        let stack = mmap(
            0,
            STACK_SIZE,
            MemoryProtFlags::READ_WRITE,
            MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS,
            None,
            0,
        );
        if stack == u64::MAX {
            println!("FAIL: mmap stack for worker {}", i);
            return -1;
        }
        let stack_top = stack + STACK_SIZE;

        // 为每个 worker 克隆一份专用函数 (通过不同的 id 参数)
        // 由于 fn 指针无法捕获 id，此处通过汇编跳板模拟
        let tid = match i {
            0 => create_thread(worker_0 as _, stack_top as _),
            1 => create_thread(worker_1 as _, stack_top as _),
            2 => create_thread(worker_2 as _, stack_top as _),
            3 => create_thread(worker_3 as _, stack_top as _),
            _ => unreachable!(),
        };
        println!("[main] worker {} tid={}", i, tid);
    }

    // 等待所有 worker 启动并阻塞在 GO 信号量上
    println!("[main] waiting 1s for workers to block on GO...");
    sleep(1000);

    // 一次性释放所有 worker
    println!("[main] signaling GO × {} ...", WORKER_COUNT);
    for _ in 0..WORKER_COUNT {
        sem_signal(go_sem);
    }

    // 等待所有 worker 完成
    println!("[main] waiting for all workers to finish...");
    for _ in 0..WORKER_COUNT {
        sem_wait(done_sem);
    }

    let final_count = COUNTER.load(Ordering::SeqCst);
    if final_count == WORKER_COUNT {
        println!("=== PASS: all {} workers completed, counter = {} ===", WORKER_COUNT, final_count);
    } else {
        println!("=== FAIL: expected {} workers, counter = {} ===", WORKER_COUNT, final_count);
    }
    0
}

// 为每个 worker 生成独立的函数 (绕开 fn 指针的闭包捕获限制)
fn worker_0() -> ! { worker(0) }
fn worker_1() -> ! { worker(1) }
fn worker_2() -> ! { worker(2) }
fn worker_3() -> ! { worker(3) }
