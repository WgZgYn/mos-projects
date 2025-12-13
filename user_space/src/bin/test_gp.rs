#![no_std]
#![no_main]

use user_space::{println, rand, write, STDOUT};

#[unsafe(no_mangle)]
fn main() -> u64 {
    println!("----- test_gp ----- start");
    let mut array: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    for v in array.iter_mut() {
        *v = rand() as u8;
    }
    print_array(&array);
    array.sort_unstable();
    print_array(&array);
    println!("----- test_gp ----- end");
    0
}

pub fn print_array(array: &[u8]) {
    let mut buffer = [0; 128];
    let n = write_array(&mut buffer, array);
    write(STDOUT, &buffer[..n]);
}

pub fn write_array(buffer: &mut [u8], array: &[u8]) -> usize {
    let mut i = 0;
    for &num in array {
        let n = write_num_u8(&mut buffer[i..], num);
        i += n;
        buffer[i] = b' ';
        i += 1;
    }
    i
}

pub fn write_num_u8(buffer: &mut [u8], mut num: u8) -> usize {
    if num == 0 {
        buffer[0] = b'0';
        return 1;
    }
    let mut w = 0;
    while num > 0 {
        buffer[w] = (num % 10) + b'0';
        num /= 10;
        w += 1;
    }
    buffer[..w].reverse();
    w
}
