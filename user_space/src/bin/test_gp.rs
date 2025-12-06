#![no_std]
#![no_main]

use core::arch::{asm, naked_asm};
use user_space::{call_sys_exit, call_sys_write_stdout};

#[unsafe(no_mangle)]
#[unsafe(naked)]
pub extern "C" fn _start() -> ! {
    naked_asm!("jmp user_main")
}

#[unsafe(no_mangle)]
pub fn user_main() {
    let message = "----- test_gp ----- start";
    call_sys_write_stdout(message.as_bytes());

    let mut array: [u8; 10] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 0];
    for v in array.iter_mut() {
        *v = get_random() as u8;
    }
    print_array(&array);
    array.sort_unstable();
    print_array(&array);

    let message = "----- test_gp ----- end";
    call_sys_write_stdout(message.as_bytes());
    call_sys_exit()
}

pub fn print_array(array: &[u8]) {
    let mut buffer = [0; 128];
    let n = write_array(&mut buffer, array);
    call_sys_write_stdout(&buffer[..n])
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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

fn get_random() -> u64 {
    let r;
    unsafe {
        asm!(
            "int 0x80",
            in("rax") 7,
            lateout("rax") r,
            clobber_abi("sysv64"),
        )
    }
    r
}
