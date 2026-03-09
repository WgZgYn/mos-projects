#![no_std]
#![no_main]
extern crate alloc;

use alloc::string::String;
use user_space::{chdir, exec, exit, fork, get_char, init_heap, open, print, println, waitpid};
use vfs::OpenFlags;

#[unsafe(no_mangle)]
fn main() -> i32 {
    init_heap();
    println!("MOS Shell");
    print!(">> ");
    let mut line = String::new();
    loop {
        let c = get_char();
        print!("{}", c as char);
        match c {
            b'\n' => {
                print!(">> ");
                match line.as_str() {
                    s if s.starts_with("./") => handler_exec(s),
                    s if s.starts_with("cd") => handler_cd(s),
                    "ls" => {
                        let pid = fork();
                        if pid == 0 {
                            exit(exec("/ls", 2) as _);
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
    let pid = fork();
    if pid == 0 {
        let code = exec(path, 2);
        if code != 0 {
            println!("execute failed");
            exit(-1);
        }
    } else {
        waitpid(pid);
        print!(">> ");
    }
}

fn handler_cd(path: &str) {
    if let Some(("cd", path)) = path.split_once(|s: char| s.is_whitespace()) {
        let dir = open(path, OpenFlags::READ | OpenFlags::DIRECTORY);
        if dir == u64::MAX {
            println!("open failed");
            return;
        }
        println!("try change work dir");
        let ok = chdir(dir);
        if ok == u64::MAX {
            println!("chdir failed");
            return;
        }
        println!("chdir success");
    }
}
