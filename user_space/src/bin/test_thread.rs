#![no_std]
#![no_main]

use user_space::{init_heap, println};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[TEST THREAD] start main");
    init_heap();
    let mut rt = green_thread::Runtime::new();
    rt.spawn(calculate_fib);
    rt.spawn(calculate_prime);
    rt.run_ready();
    0
}

fn calculate_fib() {
    println!("[TEST THREAD A] calculate fib");
    let mut a = 0;
    let mut b = 1;
    for i in 0..10 {
        let c = a + b;
        a = b;
        b = c;
        println!("[TEST THREAD A] fib({}) = {}", i, a);
        green_thread::yield_thread();
    }
    println!("[TEST THREAD A] calculate_fib finished");
}

fn calculate_prime() {
    println!("[TEST THREAD B] calculate prime");
    fn is_prime(n: i32) -> bool {
        let mut i = 2;
        while i * i <= n {
            if n % i == 0 {
                return false;
            }
            i += 1;
        }
        true
    }
    let mut c = 2;
    for i in 0..10 {
        while !is_prime(c) {
            c += 1;
        }
        println!("[TEST THREAD B] prime({}) = {}", i, c);
        c += 1;
        green_thread::yield_thread();
    }
    println!("[TEST THREAD B] calculate_prime finished");
}
