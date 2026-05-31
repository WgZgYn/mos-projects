use log::info;
use mos_arch::idt::{
    set_breakpoint_handler, set_double_fault_handler, set_general_handler,
    set_general_protection_fault_handler, set_page_fault_handler,
};
use mos_arch::make_interrupt_handler;
use mos_arch::structs::SyscallContext;
use mos_driver::serial_println;
use mos_lib::sync::KRef;
use mos_lib::utils::try_dump;
use mos_mem::{PageTableManager, PhysicalFrameManager, phys_page_copy};
use mos_platform::eoi;
use mos_platform::pic::InterruptIndex;
use mos_platform::timer::tick;
use mos_syscall::{process::sys_exit, syscall};
use mos_task::scheduler::get_preempt_count;
use mos_task::thread::ThreadControlBlock;
use mos_task::{current_tcb, schedule, scheduler_debug_info};
use x86_64::addr::VirtAddrNotValid;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::structures::paging::mapper::{MappedFrame, TranslateResult};
use x86_64::structures::paging::{
    FrameAllocator, Mapper, Page, PageTableFlags, PhysFrame, Size4KiB, Translate,
};
use x86_64::{PrivilegeLevel, VirtAddr};

// This ensures the Index is unique
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
enum IDTIndexEnum {
    Timer = InterruptIndex::Timer.as_u8(),
    Keyboard = InterruptIndex::Keyboard.as_u8(),
    Trap = 0x80,
}

impl IDTIndexEnum {
    const fn as_u8(self) -> u8 {
        self as u8
    }
}

pub fn install_handlers() {
    set_page_fault_handler(page_fault_handler);
    set_double_fault_handler(double_fault_handler);
    set_breakpoint_handler(breakpoint_handler);
    set_general_protection_fault_handler(general_protection_handler);
    set_general_handler(IDTIndexEnum::Keyboard.as_u8(), keyboard_interrupt_handler);
    set_general_handler(IDTIndexEnum::Timer.as_u8(), timer_handler);
    set_general_handler(IDTIndexEnum::Trap.as_u8(), trap_handler);
}

/// Safety: because the user thread is panic, the context is on longer being used, it's safe to iretq
fn user_space_panic_handler(_stack_frame: InterruptStackFrame) -> ! {
    sys_exit(-1)
}

pub extern "C" fn check_schedule() {
    (current_tcb().get_clear_need_schedule() && get_preempt_count() == 0).then(schedule);
}

make_interrupt_handler!(
    trap_handler,
    |frame| {
        let mut syscall_context = SyscallContext::from(frame.clone());
        syscall(&mut syscall_context);
        frame.preserved = syscall_context.preserved;
        frame.iret.rip = syscall_context.scratch.rcx;
        frame.iret.rsp = syscall_context.user_rsp;
        frame.iret.rflags = syscall_context.scratch.r11;
        frame.scratch = syscall_context.scratch;
    },
    check_schedule
);

make_interrupt_handler!(
    timer_handler,
    |_ctx| {
        let tick = unsafe { tick() };
        let need_schedule = tick.is_multiple_of(10);
        if need_schedule {
            current_tcb().set_need_schedule();
        }
        current_tcb().update_vruntime(1);
        eoi(InterruptIndex::Timer.as_u8());
    },
    check_schedule
);

extern "x86-interrupt" fn keyboard_interrupt_handler(_ctx: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    mos_driver::keyboard::add_scancode(scancode);
    eoi(InterruptIndex::Keyboard.as_u8());
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    info!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    let addr = Cr2::read();
    let current_tcb = current_tcb();
    let is_from_user = stack_frame.code_segment.rpl() == PrivilegeLevel::Ring3;
    let handler_result = page_fault_helper(error_code, addr, current_tcb);

    if let Err(e) = handler_result {
        info!("[PF] unrecoverable page fault ({})", e);
        info!("dump stack backtrace:");
        try_dump(stack_frame.stack_pointer.as_u64(), 10);
        info!("scheduler debug info:");
        scheduler_debug_info(Some(&stack_frame));

        if is_from_user {
            user_space_panic_handler(stack_frame);
        } else {
            panic!(
                "EXCEPTION: PAGE FAULT\nAccessed Address: {:?}\nError Code: {:?}\n{:#?}",
                Cr2::read(),
                error_code,
                stack_frame
            );
        }
    }
}

extern "x86-interrupt" fn general_protection_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    info!("#GP, code: {}", error_code);
    serial_println!("#GP, code: {}", error_code);
    scheduler_debug_info(Some(&stack_frame));

    if stack_frame.code_segment.rpl() == PrivilegeLevel::Ring3 {
        user_space_panic_handler(stack_frame);
    }

    panic!(
        "general protection exception: \n error_code:{}\n{:#?}",
        error_code, stack_frame
    );
}

/// return Ok, means success handler the #PF
/// return Err, means need handle the panic
fn page_fault_helper(
    error_code: PageFaultErrorCode,
    addr: Result<VirtAddr, VirtAddrNotValid>,
    current_tcb: KRef<ThreadControlBlock>,
) -> Result<(), &'static str> {
    if !error_code.contains(PageFaultErrorCode::PROTECTION_VIOLATION)
        && let Ok(addr) = addr
    {
        return current_tcb.process.mem.try_handle_frame_alloc(addr);
    }

    let mut mapper = unsafe { PageTableManager::current_mapper() };
    if error_code
        .contains(PageFaultErrorCode::PROTECTION_VIOLATION | PageFaultErrorCode::CAUSED_BY_WRITE)
        && let Ok(addr) = addr
        && let TranslateResult::Mapped { flags, frame, .. } = mapper.translate(addr)
        && flags.contains(PageTableFlags::BIT_9)
        && let MappedFrame::Size4KiB(frame) = frame
    {
        return handler_cow_page_fault(addr, frame, flags, &mut mapper);
    }

    Err("unknown page fault error")
}

fn handler_cow_page_fault(
    addr: VirtAddr,
    frame: PhysFrame,
    flags: PageTableFlags,
    mapper: &mut impl Mapper<Size4KiB>,
) -> Result<(), &'static str> {
    let count = PhysicalFrameManager.ref_get(frame);
    let page = Page::containing_address(addr);
    let new_flags = (flags - PageTableFlags::BIT_9) | PageTableFlags::WRITABLE;

    if count > 1 {
        // alloc new frame, copy data, unmap the page and dec ref
        let new_frame = PhysicalFrameManager
            .allocate_frame()
            .ok_or("couldn't allocate frame")?;
        phys_page_copy(page, new_frame);
        unsafe {
            mapper.unmap(page).unwrap().1.flush();
            mapper
                .map_to(page, new_frame, new_flags, &mut PhysicalFrameManager)
                .unwrap()
                .flush();
        }
        PhysicalFrameManager.ref_dec(frame);
        Ok(())
    } else if count == 1 {
        // inherit, undate the flags
        unsafe {
            mapper.unmap(page).unwrap().1.flush();
            mapper
                .map_to(page, frame, new_flags, &mut PhysicalFrameManager)
                .unwrap()
                .flush();
        }
        Ok(())
    } else {
        Err("COW with 0 refcount")
    }
}
