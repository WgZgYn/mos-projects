#![no_std]
#![no_main]

use user_lib::mos::fs::*;
use user_lib::{exit, fork, getpgrp, ioctl, println, setsid, tcgetattr, tcgetpgrp,
    tcsetattr, tcsetpgrp, wait4, STDIN};

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[tty-job] start");
    let group = getpgrp();
    assert!(group > 0);
    assert_eq!(tcgetpgrp(STDIN), group);

    let mut session = -1i32;
    assert_eq!(ioctl(STDIN, TIOCGSID, &mut session as *mut _ as u64), 0);
    assert!(session > 0);
    assert_eq!(ioctl(STDIN, TCGETS, 0), -14);
    assert_eq!(ioctl(FileDesc(99), TCGETS, &mut session as *mut _ as u64), -9);

    let mut original = Termios::default();
    assert_eq!(tcgetattr(STDIN, &mut original), 0);
    assert_ne!(original.c_lflag & ISIG, 0);
    assert_ne!(original.c_lflag & ICANON, 0);
    assert_eq!(original.c_cc[VINTR], 3);

    let mut raw = original;
    raw.c_lflag &= !(ICANON | ECHO);
    raw.c_cc[VMIN] = 1;
    assert_eq!(tcsetattr(STDIN, &raw), 0);
    let mut observed = Termios::default();
    assert_eq!(tcgetattr(STDIN, &mut observed), 0);
    assert_eq!(observed, raw);
    assert_eq!(tcsetattr(STDIN, &original), 0);

    let mut timed = original;
    timed.c_cc[VTIME] = 1;
    assert_eq!(tcsetattr(STDIN, &timed), -38);
    assert_eq!(tcgetattr(STDIN, &mut observed), 0);
    assert_eq!(observed, original);
    assert_eq!(tcsetpgrp(STDIN, i32::MAX), -1);

    let child = fork();
    assert!((child as i64) >= 0);
    if child == 0 {
        // A non-group-leader can create a new session. Its inherited fd still
        // names tty0, but the new session has no controlling terminal.
        assert!(setsid() > 0);
        let mut value = 0i32;
        assert_eq!(ioctl(STDIN, TIOCGPGRP, &mut value as *mut _ as u64), -25);
        assert_eq!(ioctl(STDIN, TIOCSCTTY, 0), -1);
        exit(0);
    }
    let mut status = -1;
    assert_eq!(wait4(child as i32, &mut status, 0), child as i64);
    assert_eq!(status, 0);
    assert_eq!(tcgetpgrp(STDIN), group);
    println!("[tty-job] PASS");
    0
}
