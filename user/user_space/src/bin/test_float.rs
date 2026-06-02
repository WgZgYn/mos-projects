#![no_std]
#![no_main]
extern crate alloc;

use alloc::vec::Vec;
use user_lib::{println, rand};
use libm::*;

const RAND_MAX: u32 = 0x7fffffff; // 假设 rand() 返回 0..=0x7fffffff
const PI: f64 = 3.141592653589793;

/// 生成 [0, 1) 范围的随机 f64
fn random_f64() -> f64 {
    (rand() as u32 % (RAND_MAX + 1)) as f64 / (RAND_MAX as f64 + 1.0)
}

/// 测试一组函数，对每个输入值计算并打印结果
fn test_function<F>(name: &str, inputs: &[f64], f: F)
where
    F: Fn(f64) -> f64,
{
    println!("\n=== Testing {} ===", name);
    for &x in inputs {
        let result = f(x);
        println!("{}({:.10}) = {:.10}", name, x, result);
    }
}

/// 测试二元函数 pow(x, y)
fn test_pow(inputs: &[(f64, f64)]) {
    println!("\n=== Testing pow(x, y) ===");
    for &(x, y) in inputs {
        let result = pow(x, y);
        println!("pow({:.10}, {:.10}) = {:.10}", x, y, result);
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("=== Floating-Point Math Test Suite (libm) ===");
    println!("Testing various math functions with detailed logs.\n");

    // 预定义的测试点（包含边界值、特殊值）
    let test_values = [
        0.0,
        1.0,
        -1.0,
        2.0,
        -2.0,
        PI / 6.0,      // 30°
        PI / 4.0,      // 45°
        PI / 3.0,      // 60°
        PI / 2.0,      // 90°
        PI,            // 180°
        -PI,
        2.0 * PI,
        0.5,
        -0.5,
        0.1,
        10.0,
        -10.0,
        100.0,
        0.0001,
    ];

    // ---- 三角函数 ----
    test_function("sin", &test_values, sin);
    test_function("cos", &test_values, cos);
    test_function("tan", &test_values, tan);
    test_function("asin", &test_values, asin); // asin 定义域 [-1,1]，超出返回 NaN
    test_function("acos", &test_values, acos); // acos 定义域 [-1,1]
    test_function("atan", &test_values, atan);

    // ---- 双曲函数 ----
    test_function("sinh", &test_values, sinh);
    test_function("cosh", &test_values, cosh);
    test_function("tanh", &test_values, tanh);

    // ---- 指数对数 ----
    test_function("exp", &test_values, exp);
    // log 是自然对数，输入必须 >0
    let pos_values: Vec<f64> = test_values.iter().filter(|&&x| x > 0.0).cloned().collect();
    test_function("ln", &pos_values, log);
    test_function("log10", &pos_values, log10);

    // ---- 幂函数、开方 ----
    test_function("sqrt", &test_values, |x| if x >= 0.0 { sqrt(x) } else { f64::NAN });
    // pow 需要测试各种组合
    let pow_inputs = [
        (2.0, 3.0),   // 8
        (2.0, -2.0),  // 0.25
        (4.0, 0.5),   // 2.0
        (9.0, 0.5),   // 3.0
        (16.0, 0.25), // 2.0
        (2.0, 10.0),
        (10.0, -1.0),
        (0.5, 2.0),
        (0.0, 5.0),
        (0.0, 0.0),   // 1.0 (by convention)
        (1.0, f64::INFINITY),
        (f64::INFINITY, 2.0),
        (f64::NAN, 1.0),
    ];
    test_pow(&pow_inputs);

    // ---- 绝对值、取整 ----
    test_function("fabs", &test_values, fabs);
    test_function("floor", &test_values, floor);
    test_function("ceil", &test_values, ceil);
    test_function("round", &test_values, round);

    // ---- 随机值测试（增强覆盖） ----
    println!("\n=== Random Input Tests ===");
    for _ in 0..10 {
        let x = random_f64() * 20.0 - 10.0; // [-10, 10)
        let y = random_f64() * 20.0 - 10.0;
        println!("Random pair: x = {:.6}, y = {:.6}", x, y);
        println!("  sin(x) = {:.6}", sin(x));
        println!("  cos(x) = {:.6}", cos(x));
        println!("  exp(x) = {:.6}", exp(x));
        if x > 0.0 {
            println!("  ln(x)  = {:.6}", log(x));
        } else {
            println!("  ln(x)  = nan (x <= 0)");
        }
        println!("  sqrt(|x|) = {:.6}", sqrt(fabs(x)));
        println!("  pow(x, y) = {:.6}", pow(x, y));
        println!("  hypot? Not in libm, using sqrt(x^2+y^2) = {:.6}", sqrt(x * x + y * y));
        println!("---");
    }

    println!("\n=== Test Completed Successfully ===");
    0
}