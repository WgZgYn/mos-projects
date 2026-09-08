#![no_std]
#![no_main]
extern crate alloc;

use alloc::ffi::CString;
use user_lib::{access, chmod, dup3, fcntl, getdents, link, lseek, mkdir, open, println, read,
               readlink, rename, rmdir, symlink, truncate, umask, unlink, write};
use user_lib::mos::fs::{FileDesc, O_CREAT, O_RDONLY, O_RDWR};

fn c(s: &str) -> CString {
    CString::new(s).unwrap()
}

fn expect_ok<E: core::fmt::Debug>(name: &str, r: Result<(), E>) {
    match r {
        Ok(()) => println!("[test_dir] {}: ok", name),
        Err(e) => println!("[test_dir] {}: FAIL {:?}", name, e),
    }
}

fn expect_err<E: core::fmt::Debug>(name: &str, r: Result<(), E>) {
    match r {
        Ok(()) => println!("[test_dir] {}: FAIL (expected error)", name),
        Err(e) => println!("[test_dir] {}: err {:?} (expected)", name, e),
    }
}

#[unsafe(no_mangle)]
fn main() -> i32 {
    // 1. mkdir /t and /t/sub
    expect_ok("mkdir /t", mkdir(&c("/t"), 0o755));
    expect_ok("mkdir /t/sub", mkdir(&c("/t/sub"), 0o755));
    expect_err("mkdir /t again", mkdir(&c("/t"), 0o755));

    // 2. create a file inside
    let fd = open(c("/t/a.txt").as_ref(), O_CREAT | O_RDWR).expect("open /t/a.txt");
    write(fd, b"hello dir test\n").unwrap();
    let mut buf = [0u8; 128];
    let n = getdents(fd, &mut buf);
    println!("[test_dir] getdents on file: {}", n as i64);

    // 3. rename
    expect_ok("rename /t/a.txt /t/b.txt", rename(&c("/t/a.txt"), &c("/t/b.txt")));
    expect_err("rename missing", rename(&c("/t/nope"), &c("/t/x")));

    // 4. link
    expect_ok("link /t/b.txt /t/c.txt", link(&c("/t/b.txt"), &c("/t/c.txt")));
    expect_err("link again", link(&c("/t/b.txt"), &c("/t/c.txt")));

    // 5. rmdir non-empty fails, unlink dir fails
    expect_err("rmdir /t (non-empty)", rmdir(&c("/t")));
    expect_err("unlink /t (is dir)", unlink(&c("/t")));

    // 6. symlink / readlink / access / umask / truncate / chmod
    expect_ok("symlink /t/b.txt /t/link1", symlink(&c("/t/b.txt"), &c("/t/link1")));
    let mut lbuf = [0u8; 64];
    match readlink(&c("/t/link1"), &mut lbuf) {
        Ok(n) => println!("[test_dir] readlink /t/link1: {} bytes: {}", n, core::str::from_utf8(&lbuf[..n]).unwrap_or("?")),
        Err(e) => println!("[test_dir] readlink /t/link1: FAIL {:?}", e),
    }
    match readlink(&c("/t/b.txt"), &mut lbuf) {
        Ok(_) => println!("[test_dir] readlink regular file: FAIL (expected error)"),
        Err(e) => println!("[test_dir] readlink regular file: err {:?} (expected)", e),
    }
    expect_ok("access /t/b.txt F_OK", access(&c("/t/b.txt"), 0));
    expect_err("access /missing", access(&c("/nope"), 0));

    let old_umask = umask(0o077);
    println!("[test_dir] umask old: {:o}, set 0o077", old_umask);
    let old2 = umask(old_umask);
    println!("[test_dir] umask restore -> {:o}", old2);

    // truncate: shrink then extend
    expect_ok("truncate /t/b.txt to 5", truncate(&c("/t/b.txt"), 5));
    let fd = open(c("/t/b.txt").as_ref(), O_RDONLY).expect("open for read");
    let mut rbuf = [0u8; 16];
    let n = read(fd, &mut rbuf).unwrap_or(0);
    println!("[test_dir] after truncate(5) content: {:?} ({} bytes)", &rbuf[..n], n);
    expect_ok("truncate /t/b.txt to 0", truncate(&c("/t/b.txt"), 0));
    let n = read(fd, &mut rbuf).unwrap_or(0);
    println!("[test_dir] after truncate(0) read: {} bytes", n);
    let _ = lseek(fd, 0, 0); // rewind for cleanliness

    expect_ok("chmod /t/b.txt 0644", chmod(&c("/t/b.txt"), 0o644));
    expect_err("chmod /missing", chmod(&c("/nope"), 0o644));

    // 6b. fcntl / dup3
    let fd = open(c("/t/b.txt").as_ref(), O_RDWR).expect("open for fcntl");
    let getfl = fcntl(fd, user_lib::F_GETFL, 0);
    println!("[test_dir] F_GETFL: {:#x}", getfl);
    let newfd = fcntl(fd, user_lib::F_DUPFD, 0) as usize;
    println!("[test_dir] F_DUPFD(0) -> fd {}", newfd);
    if newfd != 0 {
        let mut rbuf = [0u8; 4];
        let _ = lseek(fd, 0, 0);
        let _ = read(FileDesc(newfd), &mut rbuf);
        println!("[test_dir] read via dupfd: {:?}", &rbuf);
    }
    let d3 = dup3(fd, FileDesc(5), 0);
    println!("[test_dir] dup3(fd->5): {}", d3);
    let f_getfd = fcntl(fd, user_lib::F_GETFD, 0);
    let f_setfd = fcntl(fd, user_lib::F_SETFD, 1);
    println!("[test_dir] F_GETFD: {} F_SETFD: {}", f_getfd, f_setfd);

    // 7. cleanup: unlink link1 + b.txt + c.txt
    expect_ok("unlink /t/link1", unlink(&c("/t/link1")));
    expect_ok("unlink /t/b.txt", unlink(&c("/t/b.txt")));
    expect_ok("unlink /t/c.txt", unlink(&c("/t/c.txt")));
    expect_err("unlink missing", unlink(&c("/t/b.txt")));

    // 8. rmdir sub then t
    expect_ok("rmdir /t/sub", rmdir(&c("/t/sub")));
    expect_ok("rmdir /t", rmdir(&c("/t")));
    expect_err("rmdir /t again", rmdir(&c("/t")));

    // 9. list root to confirm cleanup
    let dir = open(c("/").as_ref(), O_RDONLY).expect("open /");
    let mut buf = [0u8; 512];
    let n = getdents(dir, &mut buf) as usize;
    println!("[test_dir] root getdents bytes: {}", n);
    if n > 0 {
        let mut pos = 0usize;
        while pos + 19 <= n {
            let reclen = u16::from_ne_bytes([buf[pos + 16], buf[pos + 17]]) as usize;
            let name_len = reclen.saturating_sub(19);
            let name = core::str::from_utf8(&buf[pos + 19..pos + 19 + name_len.saturating_sub(1)])
                .unwrap_or("?");
            println!("[test_dir] root entry: {}", name);
            pos += reclen;
        }
    }

    println!("[test_dir] ALL DONE");
    0
}
