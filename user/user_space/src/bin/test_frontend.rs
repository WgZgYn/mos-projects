#![no_std]
#![no_main]
use core::arch::asm;
use user_lib::{chdir, close, create_window, destroy_window, exit, fork, open, println, wait4};
use user_lib::mos::fs::{O_DIRECTORY, O_RDONLY};

fn raw(n: u64, a: [u64; 6]) -> i64 {
    let result: i64;
    unsafe { asm!("syscall", inlateout("rax") n => result,
        in("rdi") a[0], in("rsi") a[1], in("rdx") a[2],
        in("r10") a[3], in("r8") a[4], in("r9") a[5],
        lateout("rcx") _, lateout("r11") _); }
    result
}
fn cwd(expected: &[u8]) {
    let mut buf = [0u8; 128];
    assert_eq!(raw(79, [buf.as_mut_ptr() as u64, 128, 0, 0, 0, 0]), expected.len() as i64);
    assert_eq!(&buf[..expected.len()], expected);
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[frontend] start");
    assert_eq!(raw(80, [c"/".as_ptr() as u64, 0, 0, 0, 0, 0]), 0);
    cwd(b"/\0");
    assert_eq!(raw(80, [0; 6]), -14);
    assert_eq!(raw(80, [c"".as_ptr() as u64, 0, 0, 0, 0, 0]), -2);
    assert_eq!(raw(80, [c"/busybox".as_ptr() as u64, 0, 0, 0, 0, 0]), -20);
    assert_eq!(raw(79, [0, 128, 0, 0, 0, 0]), -14);
    let mut byte = 0u8;
    assert_eq!(raw(79, [&mut byte as *mut _ as u64, 1, 0, 0, 0, 0]), -34);
    assert_eq!(raw(80, [c"/mem".as_ptr() as u64, 0, 0, 0, 0, 0]), 0);
    cwd(b"/mem\0");
    let fd = open(c"/", O_DIRECTORY | O_RDONLY).unwrap();
    assert_eq!(chdir(fd), 0);
    close(fd);
    cwd(b"/\0");
    let window = create_window(12, 12, 32, 32).unwrap();
    let child = fork();
    assert!((child as i64) >= 0);
    if child == 0 {
        cwd(b"/\0");
        assert_eq!(raw(80, [c"/dev".as_ptr() as u64, 0, 0, 0, 0, 0]), 0);
        cwd(b"/dev\0");
        // Fork shares pixels, but never transfers ownership of the handle.
        assert_eq!(raw(user_lib::mos::syscall::Syscall::DestroyWindow as u64,
            [window.id, 0, 0, 0, 0, 0]), -2);
        let _owned = create_window(50, 50, 32, 32).unwrap();
        exit(0);
    }
    let mut status = -1;
    assert_eq!(wait4(child as i32, &mut status, 0), child as i64);
    assert_eq!(status, 0);
    cwd(b"/\0");
    unsafe { (window.pixels_ptr as *mut u32).write_volatile(0xff00ff00); }
    user_lib::window_mark_dirty(window.id, 0, 0, u64::MAX, u64::MAX);
    user_lib::window_sync();
    destroy_window(window.id);
    assert_eq!(raw(user_lib::mos::syscall::Syscall::DestroyWindow as u64,
        [window.id, 0, 0, 0, 0, 0]), -2);
    user_lib::window_sync();
    assert!(create_window(0, 0, u64::MAX, 4).is_none());
    assert_eq!(raw(user_lib::mos::syscall::Syscall::CreateWindow as u64,
        [0, 0, 32, 32, 0, 0]), -14);
    println!("[frontend] PASS");
    0
}
