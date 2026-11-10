#![no_std]
#![no_main]

use user_lib::mos::fs::FileDesc;
use user_lib::{close, dup, exit, fork, println, waitpid, write, STDOUT};

#[unsafe(no_mangle)]
fn main() -> i32 {
    let message = b"[process-baseline] stderr available\n";
    assert_eq!(write(FileDesc(2), message).unwrap(), message.len());
    let extra = dup(STDOUT).expect("duplicate stdout");
    assert!(extra.0 >= 3, "standard descriptors must be occupied");
    close(extra);

    // The legacy MOS wrapper decodes the Linux wait status for existing callers.
    for code in 1..=64 {
        let pid = fork();
        assert!((pid as i64) >= 0, "fork failed");
        if pid == 0 {
            assert_eq!(write(FileDesc(2), b"").unwrap(), 0);
            exit(code);
        }
        assert_eq!(waitpid(pid), code, "exit code publication");
        assert_eq!(waitpid(pid), -10, "child can only be reaped once");
    }
    println!("[process-baseline] PASS");
    0
}
