#![no_std]
#![no_main]
use user_lib::{execve, exit, fork, println, wait4};

/// Run on bare metal as `test_shell_probe`. Keep numbered failure statuses on
/// screen as well as serial, so an applet silently returning cannot look like PASS.
#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[shell-probe] start: builtin, cwd, external applets, repeated fork/exec");
    let pid = fork();
    if (pid as i64) < 0 { println!("[shell-probe] fork failed: {}", pid as i64); return 1; }
    if pid == 0 {
        let script = c"echo shell_builtin_ok; cd /mem || exit 11; test \"$(pwd)\" = /mem || exit 12; cd /dev || exit 13; test \"$(/busybox pwd)\" = /dev || exit 14; cd / || exit 15; /busybox ls /busybox || exit 16; /busybox date +shell_date_ok || exit 17; for n in 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16; do /busybox echo shell_exec_$n || exit 18; done; echo shell_probe_complete";
        let argv = [c"busybox".as_ptr(), c"sh".as_ptr(), c"-c".as_ptr(), script.as_ptr(), core::ptr::null()];
        let envp = [c"PATH=/".as_ptr(), c"HOME=/".as_ptr(), core::ptr::null()];
        if let Err(error) = execve(c"/busybox", &argv, &envp) {
            println!("[shell-probe] exec failed: {}", error);
            exit(19);
        }
        unreachable!();
    }
    let mut status = -1;
    let waited = wait4(pid as i32, &mut status, 0);
    println!("[shell-probe] wait={} status={:#x}", waited, status);
    if waited == pid as i64 && status == 0 { println!("[shell-probe] PASS"); 0 }
    else { println!("[shell-probe] FAIL"); 1 }
}
