#![no_std]
#![no_main]
extern crate alloc;

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec::Vec;
use user_lib::mos::fs::{O_DIRECTORY, O_RDONLY};
use user_lib::signal::{KernelSigAction, SIGINT, SIG_DFL, SIG_IGN, sigaction};
use user_lib::{chdir, close, execve, exit, fork, get_char, getpid, open, print, println,
    setpgid, tcgetattr, tcsetattr, tcsetpgrp, waitpid, STDIN};

/// The prompt for user input
const PROMPT: &str = ">> ";

#[unsafe(no_mangle)]
fn main() -> i32 {
    let shell_pid = getpid() as i32;
    // The boot shell is the first userspace job-control client. It becomes a
    // separate process group and takes tty0 before launching foreground jobs.
    let _ = setpgid(0, 0);
    let _ = tcsetpgrp(STDIN, shell_pid);
    let ignored = KernelSigAction { handler: SIG_IGN, ..KernelSigAction::default() };
    let _ = sigaction(SIGINT, Some(&ignored), None);
    println!("[shell] mos shell startup");
    println!("[shell] type a command with args, e.g. `busybox ls /` or `cat /a.txt`");
    print!("{}", PROMPT);

    let mut line = String::new();
    loop {
        let c = get_char();
        match c {
            b'\x08' => {
                // Canonical tty input normally consumes erase itself. Keep
                // this fallback for a caller that switches the tty to raw.
                line.pop();
            }
            b'\n' => {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    handle_line(trimmed);
                }
                line.clear();
                print!("{}", PROMPT);
            }
            c => {
                line.push(c as char);
            }
        }
    }
}

/// Run one command line: builtins (cd/exit) or fork+exec an external
/// program with its arguments.
fn handle_line(line: &str) {
    let mut parts = line.split_whitespace();

    // tolerate an explicit `exec` prefix for backwards compatibility
    let mut cmd = parts.next().unwrap_or("");
    if cmd == "exec" {
        cmd = parts.next().unwrap_or("");
    }
    if cmd.is_empty() {
        return;
    }
    let args: Vec<&str> = parts.collect();

    match cmd {
        "exit" => {
            println!("Goodbye!");
            exit(0);
        }
        "cd" => handle_cd(args.first().copied()),
        _ => run_program(cmd, &args),
    }
}

fn handle_cd(dir: Option<&str>) {
    let Some(dir) = dir else {
        println!("cd: missing path");
        return;
    };
    let Ok(path) = CString::new(dir) else {
        println!("cd: invalid path");
        return;
    };
    let Some(fd) = open(path.as_ref(), O_RDONLY | O_DIRECTORY) else {
        println!("cd: {}: No such directory", dir);
        return;
    };
    let result = chdir(fd) as i64;
    close(fd);
    if result < 0 {
        println!("cd: {}: failed", dir);
    }
}

fn run_program(cmd: &str, args: &[&str]) {
    // Keep the small MOS shell as a compatibility front-end while the
    // standalone Rust applets are replaced one by one. BusyBox already
    // contains the real implementations, and invoking it this way also
    // lets `sh` become the normal interactive ash entry point.
    let use_busybox = matches!(cmd, "cat" | "ls" | "sh");
    let program_name = if use_busybox { "/busybox" } else { cmd };
    let mut program_args = Vec::with_capacity(args.len() + usize::from(use_busybox));
    if use_busybox {
        program_args.push(cmd);
    }
    program_args.extend_from_slice(args);

    let mut shell_termios = user_lib::mos::fs::Termios::default();
    let saved_termios = tcgetattr(STDIN, &mut shell_termios) == 0;
    let pid = fork();
    if pid == 0 {
        let _ = setpgid(0, 0);
        // Ignored dispositions survive exec by design, so foreground programs
        // must receive the default SIGINT action instead of inheriting shell policy.
        let default = KernelSigAction { handler: SIG_DFL, ..KernelSigAction::default() };
        let _ = sigaction(SIGINT, Some(&default), None);
        // argv[0] = the command as typed (busybox dispatches on argv[1]
        // when argv[0]'s basename is "busybox")
        let mut argv_ptrs: Vec<*const i8> = Vec::new();
        let mut argv_strings: Vec<CString> = Vec::new();
        let program = match CString::new(program_name) {
            Ok(p) => p,
            Err(_) => {
                println!("{}: invalid program name", cmd);
                exit(-1);
            }
        };
        argv_strings.push(program);
        for a in &program_args {
            match CString::new(*a) {
                Ok(s) => argv_strings.push(s),
                Err(_) => {
                    println!("{}: invalid argument {:?}", cmd, a);
                    exit(-1);
                }
            }
        }
        for s in &argv_strings {
            argv_ptrs.push(s.as_ptr());
        }
        argv_ptrs.push(core::ptr::null());

        let envp = &[c"PATH=/:/bin:/usr/bin".as_ptr(), c"HOME=/".as_ptr(),
            c"TERM=linux".as_ptr(), core::ptr::null()];
        if let Err(e) = execve(&argv_strings[0], &argv_ptrs, envp) {
            println!("{}: exec failed ({})", cmd, e);
            exit(-1);
        }
        unreachable!();
    } else {
        // Both sides call setpgid: whichever wins the fork/exec race establishes
        // the group before the terminal is handed to it.
        let _ = setpgid(pid as i32, pid as i32);
        let _ = tcsetpgrp(STDIN, pid as i32);
        let ret = waitpid(pid);
        let _ = tcsetpgrp(STDIN, getpid() as i32);
        if saved_termios { let _ = tcsetattr(STDIN, &shell_termios); }
        if ret != 0 {
            println!("[exit code: {}]", ret);
        }
    }
}
