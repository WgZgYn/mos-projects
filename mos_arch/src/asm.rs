use crate::structs::ThreadContext;
use core::arch::global_asm;

global_asm!(
    "
__full_context_intr_exit:
    pop r15
    pop r14
    pop r13
    pop r12
    pop rbp
    pop rbx

    pop r11
    pop r10
    pop r9
    pop r8
    pop rsi
    pop rdi
    pop rdx
    pop rcx
    pop rax

    iretq
    "
);

global_asm!(
    "
__switch_context:
    pushfq
    pop qword ptr [rdi + 0x38]

    fxsave64 [rdi + 0x40]

    mov [rdi + 0x00], r15
    mov [rdi + 0x08], r14
    mov [rdi + 0x10], r13
    mov [rdi + 0x18], r12
    mov [rdi + 0x20], rbp
    mov [rdi + 0x28], rbx
    mov [rdi + 0x30], rsp

__switch_to:
    fxrstor64 [rsi + 0x40]

    mov r15, [rsi + 0x00]
    mov r14, [rsi + 0x08]
    mov r13, [rsi + 0x10]
    mov r12, [rsi + 0x18]
    mov rbp, [rsi + 0x20]
    mov rbx, [rsi + 0x28]
    mov rsp, [rsi + 0x30]

    push qword ptr [rsi + 0x38]
    popfq
    ret
    "
);

unsafe extern "C" {
    pub static __full_context_intr_exit: u64;
    pub static __syscall_entry: u64;
}

pub fn intr_exit_point() -> u64 {
    unsafe { (&__full_context_intr_exit) as *const _ as u64 }
}

pub fn syscall_entry_point() -> u64 {
    unsafe { (&__syscall_entry) as *const _ as u64 }
}

unsafe extern "C" {
    pub fn __switch_context(old_ctx: *mut ThreadContext, new_ctx: *const ThreadContext);
    fn __switch_to(_: *mut ThreadContext, new_ctx: *mut ThreadContext);
}

#[macro_export]
macro_rules! push_caller_regs {
    () => {
        "
        push rax
        push rcx
        push rdx
        push rdi
        push rsi
        push r8
        push r9
        push r10
        push r11
        "
    };
}

#[macro_export]
macro_rules! push_callee_regs {
    () => {
        "
        push rbx
        push rbp
        push r12
        push r13
        push r14
        push r15
        "
    };
}

#[macro_export]
macro_rules! pop_caller_regs {
    () => {
        "
        pop r11
        pop r10
        pop r9
        pop r8
        pop rsi
        pop rdi
        pop rdx
        pop rcx
        pop rax
        "
    };
}

#[macro_export]
macro_rules! pop_callee_regs {
    () => {
        "
        pop r15
        pop r14
        pop r13
        pop r12
        pop rbp
        pop rbx
        "
    };
}

#[macro_export]
macro_rules! save_full_context {
    () => {
        concat!($crate::push_caller_regs!(), $crate::push_callee_regs!())
    };
}

#[macro_export]
macro_rules! register_syscall {
    ($rust_handler:ident, |$context:ident| $body:block) => {
        pub unsafe extern "C" fn $rust_handler($context: &mut $crate::structs::SyscallContext) { $body }
        core::arch::global_asm!(
            concat!(
            "
            __syscall_entry:
            ",
            "cld",
            // swap to kernel stack and push user_rsp
            "
            swapgs
            mov gs:{user_rsp}, rsp
            mov rsp, gs:{kernel_rsp}
            push gs:{user_rsp}
            swapgs
            ",
            $crate::push_caller_regs!(),
            $crate::push_callee_regs!(),
            "
            sti
            mov rdi, rsp
            call {handler}
            ",
            $crate::pop_callee_regs!(),
            $crate::pop_caller_regs!(),
            "
            pop rsp
            sysretq
            ",
            ),
            user_rsp = const $crate::percpu::TEMP_RSP_OFFSET,
            kernel_rsp = const $crate::percpu::KERNEL_STACK_OFFSET,
            handler = sym $rust_handler
        );
    };
}

#[macro_export]
macro_rules! make_interrupt_handler {
    ($name:ident, |$ctx:ident| $body:block, $check_schedule:ident) => {
        #[unsafe(naked)]
        extern "x86-interrupt" fn $name(_: x86_64::structures::idt::InterruptStackFrame) {
            pub unsafe extern "C" fn inner($ctx: &mut $crate::structs::InterruptStack) {
                $body
            }

            core::arch::naked_asm!(
                concat!(
                    // clear the TF
                    "
                    cld
                    ",
                    // save all the context at the kernel stack
                    $crate::save_full_context!(),
                    "
                    mov rdi, rsp
                    call {handler}
                    call {check}
                    jmp {exit}
                    ",
                ),
                handler = sym inner,
                check = sym $check_schedule,
                exit = sym $crate::asm::__full_context_intr_exit,
            );
        }
    };
}

#[macro_export]
macro_rules! make_interrupt_handler_no_schedule {
    ($name:ident, |$ctx:ident| $body:block) => {
        #[unsafe(naked)]
        extern "x86-interrupt" fn $name(_: x86_64::structures::idt::InterruptStackFrame) {
            pub unsafe extern "C" fn inner($ctx: &mut $crate::structs::InterruptStack) {
                $body
            }

            core::arch::naked_asm!(
                concat!(
                    // clear the TF
                    "
                    cld
                    ",
                    // save all the context at the kernel stack
                    $crate::save_full_context!(),
                    "
                    mov rdi, rsp
                    call {handler}
                    jmp {exit}
                    ",
                ),
                handler = sym inner,
                exit = sym $crate::asm::__full_context_intr_exit,
            );
        }
    };
}
