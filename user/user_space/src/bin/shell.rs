#![no_std]
#![no_main]
extern crate alloc;

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec::Vec;
use user_lib::mos::fs::{O_DIRECTORY, O_RDONLY};
use user_lib::{chdir, execve, exit, fork, get_char, open, print, println, waitpid};

/// The prompt for user input
const PROMPT: &str = ">> ";

#[unsafe(no_mangle)]
fn main() -> i32 {
    println!("[shell] mos shell startup");
    println!("[shell] type a command with args, e.g. `busybox ls /` or `cat /a.txt`");
    print!("{}", PROMPT);

    let mut line = String::new();
    loop {
        let c = get_char();
        match c {
            b'\x08' => {
                // backspace
                if line.pop().is_some() {
                    print!("{}", c as char);
                }
            }
            b'\n' => {
                println!();
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    handle_line(trimmed);
                }
                line.clear();
                print!("{}", PROMPT);
            }
            c => {
                print!("{}", c as char);
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
    if (chdir(fd) as i64) < 0 {
        println!("cd: {}: failed", dir);
    }
}

fn run_program(cmd: &str, args: &[&str]) {
    let pid = fork();
    if pid == 0 {
        // argv[0] = the command as typed (busybox dispatches on argv[1]
        // when argv[0]'s basename is "busybox")
        let mut argv_ptrs: Vec<*const i8> = Vec::new();
        let mut argv_strings: Vec<CString> = Vec::new();
        let program = match CString::new(cmd) {
            Ok(p) => p,
            Err(_) => {
                println!("{}: invalid program name", cmd);
                exit(-1);
            }
        };
        argv_strings.push(program);
        for a in args {
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

        let envp = &[c"PATH=/".as_ptr(), core::ptr::null()];
        if let Err(e) = execve(&argv_strings[0], &argv_ptrs, envp) {
            println!("{}: exec failed ({})", cmd, e);
            exit(-1);
        }
        unreachable!();
    } else {
        let ret = waitpid(pid);
        if ret != 0 {
            println!("[exit code: {}]", ret);
        }
    }
}
