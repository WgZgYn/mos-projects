#![no_std]
#![no_main]

use core::arch::asm;
use user_lib::mos::io::Iovec;
use user_lib::mos::memory::{MemoryMapFlags, MemoryProtFlags};
use user_lib::{close, mmap, mprotect, open, pipe, println, read};

fn raw(number: u64, fd: usize, address: u64, length: u64) -> i64 {
    let result;
    unsafe {
        asm!("syscall", inlateout("rax") number => result,
            in("rdi") fd, in("rsi") address, in("rdx") length,
            lateout("rcx") _, lateout("r11") _);
    }
    result
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[write-boundary] start");
    let channel = pipe().unwrap();
    let fd = channel.pwrite.0;
    for pointer in [0, 1, u64::MAX, 0xffff_8000_0000_0000] {
        assert_eq!(raw(1, fd, pointer, 1), -14);
        assert_eq!(raw(20, fd, pointer, 1), -14);
        assert_eq!(raw(1, fd, pointer, 0), 0);
        assert_eq!(raw(20, fd, pointer, 0), 0);
    }
    assert_eq!(raw(1, usize::MAX, 0, 1), -9);
    assert_eq!(raw(1, channel.pread.0, 0, 0), -9);
    assert_eq!(raw(20, channel.pread.0, 0, 0), -9);
    assert_eq!(raw(20, fd, 0, 1025), -22);

    let memory = mmap(0, 8192, MemoryProtFlags::READ_WRITE,
        MemoryMapFlags::PRIVATE | MemoryMapFlags::ANONYMOUS, None, 0);
    assert!((memory as i64) > 0);
    unsafe { ((memory + 4095) as *mut u8).write(b'Z'); }
    assert_eq!(mprotect(memory + 4096, 4096, 0), 0);
    // One valid page fragment is committed before the inaccessible next page.
    assert_eq!(raw(1, fd, memory + 4095, 2), 1);
    let mut data = [0u8; 2];
    assert_eq!(read(channel.pread, &mut data).unwrap(), 1);
    assert_eq!(data[0], b'Z');

    let byte = [b'A'];
    let vectors = [Iovec { iov_base: byte.as_ptr() as u64, iov_len: 1 },
        Iovec { iov_base: 0, iov_len: 1 }];
    assert_eq!(raw(20, fd, vectors.as_ptr() as u64, 2), 1);
    assert_eq!(read(channel.pread, &mut data).unwrap(), 1);
    assert_eq!(data[0], b'A');
    let overflow = [Iovec { iov_base: 0, iov_len: i64::MAX as u64 },
        Iovec { iov_base: 0, iov_len: 1 }];
    assert_eq!(raw(20, fd, overflow.as_ptr() as u64, 2), -22);

    // An invalid descriptor array is rejected before ANY file output, unlike
    // a later invalid data buffer after earlier vectors have been written.
    unsafe { ((memory + 4096 - 16) as *mut Iovec).write(vectors[0]); }
    assert_eq!(raw(20, fd, memory + 4096 - 16, 2), -14);
    assert_eq!(raw(1, fd, byte.as_ptr() as u64, 1), 1);
    assert_eq!(read(channel.pread, &mut data).unwrap(), 1);

    // Descriptor alignment is not a userspace ABI requirement.
    unsafe { ((memory + 1) as *mut Iovec).write_unaligned(vectors[0]); }
    assert_eq!(raw(20, fd, memory + 1, 1), 1);
    assert_eq!(read(channel.pread, &mut data).unwrap(), 1);
    let executable = open(c"/test_write_boundary", 0).unwrap();
    let file_page = mmap(0, 4096, MemoryProtFlags::READ,
        MemoryMapFlags::PRIVATE, Some(executable), 0);
    assert!((file_page as i64) > 0);
    // No userspace load touches this mapping first: copyin must populate it.
    assert_eq!(raw(1, fd, file_page, 4), 4);
    let mut magic = [0; 4];
    assert_eq!(read(channel.pread, &mut magic).unwrap(), 4);
    assert_eq!(magic, [0x7f, b'E', b'L', b'F']);
    close(executable);
    close(channel.pread);
    close(channel.pwrite);
    println!("[write-boundary] PASS");
    0
}
