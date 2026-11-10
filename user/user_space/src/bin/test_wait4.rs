#![no_std]
#![no_main]

use core::arch::asm;
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::mos::process::{WNOHANG, WUNTRACED, exit_code, exited};
use user_lib::{close, exit, fork, mmap, mprotect, pipe, println, read, wait4, write};

// Raw addresses are intentional: constructing a Rust reference to an invalid
// or unaligned pointer would itself be UB before the syscall is even tested.
fn raw_wait(pid: i32, status: u64, options: u32, usage: u64) -> i64 {
    let result: i64;
    unsafe {
        asm!("syscall", inlateout("rax") 61u64 => result,
            in("rdi") pid as u64, in("rsi") status, in("rdx") options as u64,
            in("r10") usage, lateout("rcx") _, lateout("r11") _);
    }
    result
}

fn child(code: i32) -> i32 {
    let pid = fork() as i64;
    assert!(pid >= 0);
    if pid == 0 { exit(code); }
    pid as i32
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    let mut status = 0x1234;
    assert_eq!(wait4(-1, &mut status, WNOHANG), -10);
    assert_eq!(wait4(-1, &mut status, 0x8000), -22);
    assert_eq!(wait4(0, &mut status, 0), -10); // no child in the caller's group
    assert_eq!(raw_wait(-1, 0, 0, 1), -38);

    // A pipe gates the child deterministically; no timing assumption is used
    // to assert WNOHANG on a still-running child, on either one or four CPUs.
    let gate = pipe().unwrap();
    let pid = fork() as i64;
    assert!(pid >= 0);
    if pid == 0 {
        close(gate.pwrite);
        let mut byte = [0];
        assert_eq!(read(gate.pread, &mut byte).unwrap(), 1);
        exit(37);
    }
    close(gate.pread);
    assert_eq!(wait4(pid as i32, &mut status, WNOHANG), 0);
    assert_eq!(wait4(-1, &mut status, WNOHANG), 0);
    assert_eq!(status, 0x1234, "WNOHANG must leave status untouched");
    write(gate.pwrite, b"x").unwrap();
    close(gate.pwrite);
    // Interactive ash always requests stopped children too. MOS has no stop
    // states yet, but WUNTRACED must not turn a normal exit wait into EINVAL.
    assert_eq!(wait4(pid as i32, &mut status, WUNTRACED), pid);
    assert!(exited(status));
    assert_eq!(exit_code(status), 37);
    assert_eq!(wait4(pid as i32, &mut status, 0), -10);

    let a = child(7);
    let b = child(259);
    assert_ne!(a, b);
    let first = wait4(-1, &mut status, 0) as i32;
    assert!(first == a || first == b);
    assert_eq!(exit_code(status), if first == a { 7 } else { 3 });
    let second = wait4(-1, &mut status, 0) as i32;
    assert_eq!(second, if first == a { b } else { a });
    assert_eq!(exit_code(status), if second == a { 7 } else { 3 });

    let pid = child(42);
    for address in [1, u64::MAX, 0x8000_0000_0000, 0xffff_8000_0000_0000] {
        assert_eq!(raw_wait(pid, address, 0, 0), -14);
    }
    let newer = child(43);
    assert_ne!(newer, pid, "an unreaped zombie must retain its PID identity");
    // Failed output must not consume the zombie; retry with a valid address.
    assert_eq!(wait4(pid, &mut status, 0), pid as i64);
    assert_eq!(exit_code(status), 42);
    assert_eq!(wait4(newer, &mut status, 0), newer as i64);
    assert_eq!(exit_code(status), 43);

    let memory = mmap(0, 8192, MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS, None, 0);
    assert!((memory as i64) > 0);
    let pid = child(55);
    assert_eq!(mprotect(memory + 4096, 4096, 1), 0);
    assert_eq!(raw_wait(pid, memory + 4094, 0, 0), -14);
    assert_eq!(mprotect(memory + 4096, 4096, 3), 0);
    // Unaligned output crosses two untouched anonymous demand pages.
    assert_eq!(raw_wait(pid, memory + 4094, 0, 0), pid as i64);
    assert_eq!(unsafe { (memory as *const u8).add(4094).cast::<i32>().read_unaligned() }, 55 << 8);

    // Keep a child alive on the old page while wait4 writes into the parent's
    // COW mapping. The child must retain the original bytes after the write.
    unsafe { (memory as *mut i32).write(0x12345678); }
    let gate = pipe().unwrap();
    let observer = fork() as i64;
    assert!(observer >= 0);
    if observer == 0 {
        close(gate.pwrite);
        let mut byte = [0];
        assert_eq!(read(gate.pread, &mut byte).unwrap(), 1);
        assert_eq!(unsafe { (memory as *const i32).read_volatile() }, 0x12345678);
        exit(0);
    }
    close(gate.pread);
    let pid = child(61);
    assert_eq!(raw_wait(pid, memory, 0, 0), pid as i64);
    assert_eq!(unsafe { (memory as *const i32).read_volatile() }, 61 << 8);
    write(gate.pwrite, b"x").unwrap();
    close(gate.pwrite);
    assert_eq!(wait4(observer as i32, &mut status, 0), observer);
    assert_eq!(status, 0);

    let pid = fork() as i64;
    assert!(pid >= 0);
    if pid == 0 {
        // set_tid_address accepts the address; an exit-time EFAULT is ignored.
        unsafe {
            asm!("syscall", inlateout("rax") 218u64 => _, in("rdi") u64::MAX,
                lateout("rcx") _, lateout("r11") _);
        }
        exit(73);
    }
    assert_eq!(wait4(pid as i32, &mut status, 0), pid);
    assert_eq!(exit_code(status), 73);

    assert_eq!(raw_wait(child(-1), 0, 0, 0) > 0, true);
    assert_eq!(wait4(-1, &mut status, WNOHANG), -10);
    println!("[wait4] PASS");
    0
}
