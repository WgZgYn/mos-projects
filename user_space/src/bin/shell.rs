#![no_std]
#![no_main]
extern crate alloc;

use alloc::string::String;
use user_space::{exec, exit, fork, get_char, init_heap, print, println, waitpid};

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
                    s if s.starts_with("./") => {
                        let pid = fork();
                        if pid == 0 {
                            let code = exec(s, 2);
                            if code != 0 {
                                println!("execute failed");
                                exit(-1);
                            }
                        } else {
                            println!("waitpid");
                            waitpid(pid);
                            print!(">> ");
                        }
                    }
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
