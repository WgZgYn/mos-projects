#![no_std]


extern crate alloc;

use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;
use core::arch::{asm, naked_asm};
use core::ops::DerefMut;
use core::ptr::null_mut;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum State {
    Available,
    Running,
    Ready,
}

const DEFAULT_STACK_SIZE: usize = 1024 * 1024 * 2;
pub const MAX_THREADS: usize = 4;

static mut RUNTIME_MUT: *mut Runtime = null_mut();

#[derive(Debug, Default, Clone)]
#[repr(C)]
pub struct ThreadContext {
    pub rsp: u64,
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub rbx: u64,
    pub rbp: u64,
}

pub struct Thread {
    stack: Vec<u8>,
    ctx: ThreadContext,
    state: State,
}

struct Runtime {
    threads: Vec<Thread>,
    current: usize,
}

impl Thread {
    fn new() -> Self {
        Thread {
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Available,
        }
    }
}
impl Runtime {
    pub fn new() -> Box<Self> {
        let base_thread = Thread {
            stack: vec![0u8; DEFAULT_STACK_SIZE],
            ctx: ThreadContext::default(),
            state: State::Running,
        };

        let mut threads = vec![base_thread];
        let mut available_threads: Vec<Thread> = (0..MAX_THREADS).map(|_| Thread::new()).collect();
        threads.append(&mut available_threads); // one main with threads

        let mut runtime = Box::new(Runtime {
            threads,
            current: 0,
        });

        unsafe {
            RUNTIME_MUT = runtime.deref_mut() as *mut Runtime;
        }

        runtime
    }

    pub unsafe fn global() -> Option<&'static mut Self> {
        unsafe { RUNTIME_MUT.as_mut() }
    }

    pub fn run(&mut self) -> ! {
        while self.t_yield() {}
        loop {
            core::hint::spin_loop();
        }
    }

    fn t_return(&mut self) {
        if self.current != 0 {
            self.threads[self.current].state = State::Available;
            self.t_yield();
        }
    }

    #[inline(never)]
    fn t_yield(&mut self) -> bool {
        let mut pos = self.current;
        while self.threads[pos].state != State::Ready {
            pos = (pos + 1) % self.threads.len();
            if pos == self.current {
                return false;
            }
        }

        if self.threads[self.current].state != State::Available {
            self.threads[self.current].state = State::Ready;
        }

        self.threads[pos].state = State::Running;
        let old_pos = self.current;
        self.current = pos;

        unsafe {
            let old: *mut ThreadContext = &mut self.threads[old_pos].ctx;
            let new: *const ThreadContext = &self.threads[pos].ctx;
            asm!("call switch", in("rdi") old, in("rsi") new, clobber_abi("C"));
        }
        !self.threads.is_empty()
    }

    pub fn spawn(&mut self, f: fn()) {
        let available = self
            .threads
            .iter_mut()
            .find(|t| t.state == State::Available)
            .expect("no available threads");

        let size = available.stack.len();

        unsafe {
            let s_ptr = available.stack.as_mut_ptr().offset(size as isize);
            let s_ptr = (s_ptr as usize & !15) as *mut u8;
            core::ptr::write(s_ptr.offset(-16) as *mut u64, guard as u64);
            core::ptr::write(s_ptr.offset(-24) as *mut u64, skip as u64);
            core::ptr::write(s_ptr.offset(-32) as *mut u64, f as u64);
        }

        available.state = State::Ready;
    }
}

fn guard() {
    unsafe {
        (*RUNTIME_MUT).t_return();
    }
}

#[unsafe(naked)]
unsafe extern "C" fn skip() {
    naked_asm!("ret")
}

pub fn yield_thread() {
    unsafe {
        (*RUNTIME_MUT).t_yield();
    }
}

#[unsafe(naked)]
#[unsafe(no_mangle)]
unsafe extern "C" fn switch() {
    naked_asm!(
        "mov [rdi + 0x00], rsp",
        "mov [rdi + 0x08], r15",
        "mov [rdi + 0x10], r14",
        "mov [rdi + 0x18], r13",
        "mov [rdi + 0x20], r12",
        "mov [rdi + 0x28], rbx",
        "mov [rdi + 0x30], rbp",
        "mov rsp, [rsi + 0x00]",
        "mov r15, [rsi + 0x08]",
        "mov r14, [rsi + 0x10]",
        "mov r13, [rsi + 0x18]",
        "mov r12, [rsi + 0x20]",
        "mov rbx, [rsi + 0x28]",
        "mov rbp, [rsi + 0x30]",
        "ret",
    )
}
