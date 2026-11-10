#![no_std]
#![no_main]
use core::arch::asm;
use user_lib::mos::fs::FileDesc;
use user_lib::{close, execve, exit, fork, getpid, pipe, println, read, wait4, write};
fn raw(nr: u64, a: i64, b: i64) -> i64 {
    let result;
    unsafe { asm!("syscall", inlateout("rax") nr => result, in("rdi") a,
        in("rsi") b, lateout("rcx") _, lateout("r11") _); }
    result
}
fn send(fd: FileDesc) { assert_eq!(write(fd, &[1]).unwrap(), 1); }
fn recv(fd: FileDesc) { assert_eq!(read(fd, &mut [0]).unwrap(), 1); }
fn reap(pid: u64) {
    let mut status = -1;
    assert_eq!(wait4(pid as i32, &mut status, 0), pid as i64);
    assert_eq!(status, 0);
}
#[unsafe(no_mangle)]
fn main(argc: usize) -> i32 {
    if argc == 2 {
        assert_eq!(raw(111, 0, 0), getpid() as i64);
        assert_ne!(raw(124, 0, 0), getpid() as i64);
        send(FileDesc(6));
        recv(FileDesc(3));
        return 0;
    }
    println!("[job-identity] start");
    let parent = getpid() as i64;
    let inherited_group = raw(111, 0, 0);
    let inherited_session = raw(124, 0, 0);
    assert!(inherited_group > 0 && inherited_session > 0);
    assert_eq!(raw(121, parent, 0), inherited_group);
    assert_eq!(raw(121, i32::MAX as i64, 0), -3);
    assert_eq!(raw(109, 0, -1), -22);
    assert_eq!(raw(109, 0, i32::MAX as i64), -1);

    let gate = pipe().unwrap();
    let ready = pipe().unwrap();
    assert_eq!(gate.pread.0, 3);
    assert_eq!(ready.pwrite.0, 6);
    let leader = fork();
    assert!((leader as i64) >= 0);
    if leader == 0 {
        assert_eq!(raw(111, 0, 0), inherited_group);
        assert_eq!(raw(124, 0, 0), inherited_session);
        assert_eq!(raw(109, parent, parent), -3); // cannot change parent
        send(ready.pwrite);
        recv(gate.pread);
        assert_eq!(raw(111, 0, 0), getpid() as i64);
        assert_eq!(raw(112, 0, 0), -1); // group leader cannot setsid
        exit(0);
    }
    recv(ready.pread);
    assert_eq!(raw(109, leader as i64, 0), 0);
    assert_eq!(raw(121, leader as i64, 0), leader as i64);
    let member_gate = pipe().unwrap();
    let member = fork();
    assert!((member as i64) >= 0);
    if member == 0 {
        assert_eq!(raw(109, 0, leader as i64), 0);
        send(ready.pwrite);
        // Parent reaps the leader before releasing us.
        recv(member_gate.pread);
        assert_eq!(raw(111, 0, 0), leader as i64);
        assert_eq!(raw(112, 0, 0), getpid() as i64);
        assert_eq!(raw(124, 0, 0), getpid() as i64);
        assert_eq!(raw(111, 0, 0), getpid() as i64);
        assert_eq!(raw(109, 0, 0), -1); // session leader
        send(ready.pwrite);
        recv(member_gate.pread);
        exit(0);
    }
    recv(ready.pread);
    send(gate.pwrite);
    reap(leader);
    assert_eq!(raw(121, member as i64, 0), leader as i64);
    send(member_gate.pwrite);
    recv(ready.pread);
    assert_eq!(raw(109, member as i64, inherited_group), -1);
    send(member_gate.pwrite);
    reap(member);
    close(member_gate.pread);
    close(member_gate.pwrite);

    let child = fork();
    assert!((child as i64) >= 0);
    if child == 0 {
        assert_eq!(raw(109, 0, 0), 0);
        let args = [c"/test_job_identity".as_ptr(), c"exec".as_ptr(), core::ptr::null()];
        execve(c"/test_job_identity", &args, &[core::ptr::null()]).unwrap();
        unreachable!();
    }
    recv(ready.pread);
    assert_eq!(raw(109, child as i64, inherited_group), -13);
    send(gate.pwrite);
    reap(child);
    for fd in [gate.pread, gate.pwrite, ready.pread, ready.pwrite] { close(fd); }
    println!("[job-identity] PASS");
    0
}
