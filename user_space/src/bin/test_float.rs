#![no_std]
#![no_main]

use user_space::{println, rand, sleep};

const A: f64 = 123.456;
const B: f64 = 456.789;
const C: f64 = A + B;

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("--- test_float ---");
    for _ in 0..10 {
        let r = rand();
        let a = A + (r % 10) as f64;
        let b = B;
        let s = libm::sin(a);
        sleep(5);
        if s > 0.5 {
            println!("sin(a+r%10) > 0.5")
        }
        assert_eq!(a + b, C + (r % 10) as f64);
    }
    0
}
