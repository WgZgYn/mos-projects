#![no_std]
#![no_main]
use core::arch::asm;
use user_lib::mos::io::Iovec;
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{close, exit, fork, mmap, mprotect, open, pipe, println, read, wait4, write};
fn raw(number: u64, fd: usize, address: u64, count: u64) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") number => result,
        in("rdi") fd, in("rsi") address, in("rdx") count,
        lateout("rcx") _, lateout("r11") _); }
    result
}
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[read-boundary] start");
    let channel = pipe().unwrap();
    let fd = channel.pread.0;
    assert_eq!(raw(0, fd, 0, 0), 0);
    assert_eq!(raw(19, fd, 0, 0), 0);
    assert_eq!(raw(0, channel.pwrite.0, 0, 0), -9);
    assert_eq!(raw(19, fd, 0, 1025), -22);
    assert_eq!(write(channel.pwrite, b"AB").unwrap(), 2);
    for address in [0, 1, u64::MAX, 0xffff_8000_0000_0000] {
        assert_eq!(raw(0, fd, address, 1), -14);
        assert_eq!(raw(19, fd, address, 1), -14);
    }
    let memory = mmap(0, 8192, MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS, None, 0);
    assert!((memory as i64) > 0);
    assert_eq!(mprotect(memory + 4096, 4096, 1), 0);
    assert_eq!(raw(0, fd, memory + 4096, 1), -14);
    let vectors = [Iovec { iov_base: memory, iov_len: 1 },
        Iovec { iov_base: 0, iov_len: 1 }];
    assert_eq!(raw(19, fd, vectors.as_ptr() as u64, 2), 1);
    assert_eq!(unsafe { (memory as *const u8).read() }, b'A');
    let mut byte = [0];
    assert_eq!(read(channel.pread, &mut byte).unwrap(), 1);
    assert_eq!(byte, [b'B']); // failed requests did not consume the pipe

    assert_eq!(write(channel.pwrite, b"C").unwrap(), 1);
    let vectors = [Iovec { iov_base: memory, iov_len: 1 },
        Iovec { iov_base: memory + 1, iov_len: 1 }];
    // Writer remains open: the second vector must not wait for more data.
    assert_eq!(raw(19, fd, vectors.as_ptr() as u64, 2), 1);
    assert_eq!(unsafe { (memory as *const u8).read() }, b'C');
    let overflow = [Iovec { iov_base: memory, iov_len: i64::MAX as u64 },
        Iovec { iov_base: memory, iov_len: 1 }];
    assert_eq!(raw(19, fd, overflow.as_ptr() as u64, 2), -22);
    unsafe { ((memory + 4096 - 16) as *mut Iovec).write(vectors[0]); }
    assert_eq!(mprotect(memory + 4096, 4096, 0), 0);
    assert_eq!(write(channel.pwrite, b"D").unwrap(), 1);
    assert_eq!(raw(19, fd, memory + 4096 - 16, 2), -14);
    assert_eq!(read(channel.pread, &mut byte).unwrap(), 1);
    assert_eq!(byte, [b'D']);

    let executable = open(c"/test_read_boundary", 0).unwrap();
    assert_eq!(raw(0, executable.0, 0, 4), -14);
    let mut magic = [0; 4];
    assert_eq!(read(executable, &mut magic).unwrap(), 4);
    assert_eq!(magic, [0x7f, b'E', b'L', b'F']); // failed read preserved offset
    let file_page = mmap(0, 4096, MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE, Some(executable), 0);
    assert!((file_page as i64) > 0);
    assert_eq!(write(channel.pwrite, b"EF").unwrap(), 2);
    assert_eq!(raw(0, fd, file_page, 2), 2); // absent destination file page prepared outside pipe lock
    assert_eq!(unsafe { core::slice::from_raw_parts(file_page as *const u8, 2) }, b"EF");
    close(executable);

    unsafe { (memory as *mut u8).write(99); }
    let pid = fork();
    assert!((pid as i64) >= 0);
    if pid == 0 {
        close(channel.pwrite);
        assert_eq!(raw(0, fd, memory, 1), 1);
        assert_eq!(unsafe { (memory as *const u8).read() }, b'G');
        exit(0);
    }
    assert_eq!(write(channel.pwrite, b"G").unwrap(), 1);
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, 0);
    assert_eq!(unsafe { (memory as *const u8).read() }, 99);
    close(channel.pwrite);
    assert_eq!(raw(0, fd, memory, 1), 0);
    close(channel.pread);
    println!("[read-boundary] PASS");
    0
}
