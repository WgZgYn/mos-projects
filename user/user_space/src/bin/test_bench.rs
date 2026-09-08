#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{clock_freq, clock_us, create_thread, exit, mmap, println, sem_create,
               sem_signal, sem_wait, sleep};

const PRIME_LIMIT: u64 = 3_000_000;
const MAX_THREADS: usize = 6;
const STACK_SIZE: u64 = 4096 * 4;

/// 将整个 range 划分为 N 个细粒度 chunk，各线程交错取用
const TOTAL_CHUNKS: u64 = 500;

static START_SEM: AtomicU32 = AtomicU32::new(0);
static DONE_SEM: AtomicU32 = AtomicU32::new(0);
static RESULTS: [AtomicU64; MAX_THREADS] = [const { AtomicU64::new(0) }; MAX_THREADS];
static CHUNK_SIZE: AtomicU64 = AtomicU64::new(0);
static THREAD_COUNT: AtomicU32 = AtomicU32::new(0);
static NEXT_TID: AtomicU32 = AtomicU32::new(0);

fn us_to_ms(us: u64) -> u64 {
    us / 1000
}

fn is_prime(n: u64) -> bool {
    if n < 2 { return false; }
    if n == 2 || n == 3 { return true; }
    if n % 2 == 0 || n % 3 == 0 { return false; }
    let mut i = 5;
    while i * i <= n {
        if n % i == 0 || n % (i + 2) == 0 { return false; }
        i += 6;
    }
    true
}

fn count_primes_range(start: u64, end: u64) -> u64 {
    let mut count = 0;
    let mut n = start;
    while n <= end {
        if is_prime(n) { count += 1; }
        n += 1;
    }
    count
}

/// 交错分配: 线程 i 取 chunk i, i+N, i+2N, i+3N, ...
/// 每个线程均匀拿到大小数值混合的 chunk
fn worker_entry() -> ! {
    let id = NEXT_TID.fetch_add(1, Ordering::SeqCst) as usize;
    let start_sem = START_SEM.load(Ordering::Acquire) as u64;
    let done_sem = DONE_SEM.load(Ordering::Acquire) as u64;
    let nthreads = THREAD_COUNT.load(Ordering::Acquire) as u64;
    let chunk_sz = CHUNK_SIZE.load(Ordering::Acquire);

    sem_wait(start_sem);

    let mut count = 0u64;
    let mut c = id as u64;
    while c < TOTAL_CHUNKS {
        let my_start = 2 + c * chunk_sz;
        let my_end = if c == TOTAL_CHUNKS - 1 {
            PRIME_LIMIT
        } else {
            my_start + chunk_sz - 1
        };
        count += count_primes_range(my_start, my_end);
        c += nthreads; // 下一个分配给当前线程的 chunk
    }

    RESULTS[id].store(count, Ordering::Release);
    sem_signal(done_sem);
    exit(0)
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let freq = clock_freq();
    let chunk_sz = (PRIME_LIMIT - 1) / TOTAL_CHUNKS;
    println!("PM-timer freq={}Hz  range={} chunks={} chunk={}",
        freq, PRIME_LIMIT, TOTAL_CHUNKS, chunk_sz);

    // 单线程基线
    println!("--- Single-threaded ---");
    let t1 = clock_us();
    let single_result = count_primes_range(2, PRIME_LIMIT);
    let t2 = clock_us();
    let single_us = t2 - t1;
    println!("SINGLE: result={} {}us (≈{}ms)",
        single_result, single_us, us_to_ms(single_us));

    // 多线程: 2, 4, 6
    for &nthreads in &[2u32, 4, 6] {
        NEXT_TID.store(0, Ordering::Release);
        for i in 0..MAX_THREADS { RESULTS[i].store(0, Ordering::Release); }

        let start_sem = sem_create(0);
        let done_sem = sem_create(0);
        START_SEM.store(start_sem as u32, Ordering::Release);
        DONE_SEM.store(done_sem as u32, Ordering::Release);
        CHUNK_SIZE.store(chunk_sz, Ordering::Release);
        THREAD_COUNT.store(nthreads, Ordering::Release);

        for _ in 0..nthreads {
            let stack = mmap(0, STACK_SIZE,
                MemoryProtFlags::READ_WRITE,
                MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS,
                None, 0);
            if (stack as i64) < 0 { println!("mmap fail"); return -1; }
            create_thread(worker_entry as _, (stack + STACK_SIZE) as _);
        }

        sleep(300);

        let t1 = clock_us();
        for _ in 0..nthreads { sem_signal(start_sem); }
        for _ in 0..nthreads { sem_wait(done_sem); }
        let t2 = clock_us();

        let multi_us = t2 - t1;
        let mut total = 0u64;
        for i in 0..nthreads as usize { total += RESULTS[i].load(Ordering::Acquire); }

        let speedup = single_us as f64 / multi_us as f64;
        println!("MULTI-{}: result={} {}us (≈{}ms) speedup≈{:.2}x",
            nthreads, total, multi_us, us_to_ms(multi_us), speedup);
        if total != single_result {
            println!("  MISMATCH! single={} multi={}", single_result, total);
        }
    }

    println!("--- Done ---");
    0
}
