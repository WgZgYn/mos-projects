#![no_std]
#![no_main]
extern crate alloc;

use alloc::ffi::CString;
use alloc::string::String;
use alloc::vec::Vec;
use core::ffi::{CStr, c_char};
use mos_lib::ffi::NullEndPointerArray;
use user_space::{
    chdir, exec, execve, exit, fork, get_char, init_heap, open, print, println, waitpid,
};
use vfs::OpenFlags;

#[unsafe(no_mangle)]
fn main(argc: usize, argv: *const *const c_char, envp: *const *const c_char) -> i32 {
    init_heap();
    println!("MOS Shell");

    unsafe {
        let argv = core::slice::from_raw_parts(argv, argc);
        let envp = NullEndPointerArray::from_ptr(envp);
        let envp = envp.as_array();
        argv.iter().enumerate().for_each(|(i, &arg)| {
            println!("arg[{}] {:?}", i, CStr::from_ptr(arg));
        });
        envp.iter().enumerate().for_each(|(i, &env)| {
            println!("env[{}] {:?}", i, CStr::from_ptr(env));
        })
    }

    print!(">> ");
    let mut line = String::new();
    loop {
        let c = get_char();
        print!("{}", c as char);
        match c {
            b'\n' => {
                print!(">> ");
                match line.as_str() {
                    s if s.starts_with("./") || s.starts_with("exec") => handler_exec(s),
                    s if s.starts_with("cd") => handler_cd(s),
                    s if s.starts_with("cat") => handler_cat(s),
                    "ls" => {
                        let pid = fork();
                        if pid == 0 {
                            match exec(c"/ls") {
                                Ok(_) => exit(0),
                                Err(e) => exit(e as _),
                            }
                        } else {
                            waitpid(pid);
                        }
                    }
                    "exit" => {
                        println!("Goodbye!");
                        exit(0);
                    }
                    _ => {}
                }
                line.clear();
            }
            _ => line.push(c as char),
        }
    }
}

fn handler_exec(path: &str) {
    let mut path = path;
    if let Some(("exec", name)) = path.split_once(|s: char| s.is_whitespace()) {
        path = name;
    }
    let pid = fork();
    if pid == 0 {
        let Ok(path) = CString::new(path) else {
            println!("Failed to convert path to CString");
            return;
        };
        if let Err(e) = exec(&path) {
            println!("execute failed");
            exit(e as _);
        }
    } else {
        waitpid(pid);
        print!(">> ");
    }
}

fn handler_cd(path: &str) {
    if let Some(("cd", path)) = path.split_once(|s: char| s.is_whitespace()) {
        let Some(dir) = open(path, OpenFlags::READ | OpenFlags::DIRECTORY) else {
            println!("Failed to open directory");
            return;
        };
        println!("try change work dir");
        let ok = chdir(dir);
        if ok == u64::MAX {
            println!("chdir failed");
            return;
        }
        println!("chdir success");
    }
}

fn handler_cat(line: &str) {
    let mut split = line.trim().split_whitespace();
    let Some(cat) = split.next() else {
        println!("handler error");
        return;
    };

    let mut buffer = Vec::new();
    let mut args = Vec::new();

    if let Ok(s) = CString::new(cat) {
        buffer.push(s);
    } else {
        println!("Failed to convert path to CString");
        return;
    }

    for item in split {
        let Ok(s) = CString::new(item) else {
            println!("args illegal");
            return;
        };

        buffer.push(s);
    }
    buffer.iter().for_each(|s| args.push(s.as_ref().as_ptr()));
    args.push(core::ptr::null());

    let Some(f) = open(cat, OpenFlags::CREATE | OpenFlags::READ) else {
        println!("no such file or directory");
        return;
    };
    let pid = fork();
    if pid == 0 {
        if let Err(e) = execve(c"/cat".as_ref(), &args, &[core::ptr::null()]) {
            println!("execve cat failed with error {}", e);
            return;
        }
    } else {
        waitpid(pid);
        print!(">> ");
    }
}
