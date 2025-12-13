use core::arch::asm;

fn syscall(no: u64, args: [u64; 3]) -> u64 {
    let ret: u64;
    unsafe {
        asm!("int 0x80",
        in("rax") no,
        in("rdi") args[0],
        in("rsi") args[1],
        in("rdx") args[2],
        lateout("rax") ret,
        )
    }
    ret
}

pub fn sys_write(fd: u64, buffer: &[u8]) -> u64 {
    syscall(1, [fd, buffer.as_ptr() as _, buffer.len() as u64])
}

pub fn sys_exit(code: u64) -> u64 {
    syscall(60, [code, 0, 0])
}

pub fn sys_rand() -> u64 {
    syscall(7, [0, 0, 0])
}

pub fn sys_yield() -> u64 {
    syscall(24, [0, 0, 0])
}