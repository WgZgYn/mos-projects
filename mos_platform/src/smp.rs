//! SMP IPI (Inter-Processor Interrupt) 和 TLB shootdown 基础设施

use crate::acpi::APIC;
use core::sync::atomic::{AtomicBool, Ordering};

/// IPI 向量定义 — 需要与 IDT 中的处理函数对应
pub const IPI_TLB_SHOOTDOWN: u8 = 0x30; // vector 48
pub const IPI_RESCHEDULE: u8 = 0x31;     // vector 49

static TLB_SHOOTDOWN_PENDING: AtomicBool = AtomicBool::new(false);

/// 向所有其他 CPU 广播 IPI（Shorthand: All-Excluding-Self）
pub fn broadcast_ipi(vector: u8) {
    let acpi = APIC.get().expect("APIC not initialized");
    let x2apic = crate::is_x2apic_enabled();
    let xapic_base = acpi.xapic_base;

    // ICR: Shorthand=All-Excl-Self(3), Fixed Mode, Assert, Physical Destination
    let icr: u64 = (vector as u64) | (3 << 18) | (1 << 14);

    unsafe {
        let mut lapic_lock = acpi.bsp_lapic.lock();
        while lapic_lock.get_ipi_delivery_status() {
            core::hint::spin_loop();
        }
        crate::write_raw_icr(xapic_base, icr, x2apic);
    }
}

/// TLB shootdown: 向所有其他核发送 flush TLB 请求
pub fn tlb_shootdown() {
    TLB_SHOOTDOWN_PENDING.store(true, Ordering::Release);
    broadcast_ipi(IPI_TLB_SHOOTDOWN);
}

/// 检查是否需要 TLB shootdown（供异常处理使用）
pub fn is_tlb_shootdown_pending() -> bool {
    TLB_SHOOTDOWN_PENDING.load(Ordering::Acquire)
}

/// IPI 处理入口 — 从 IDT handler 调用
pub fn handle_ipi(vector: u8) {
    match vector {
        IPI_TLB_SHOOTDOWN => {
            // 重载 CR3 刷新本核 TLB
            unsafe {
                let (frame, flags) = x86_64::registers::control::Cr3::read();
                x86_64::registers::control::Cr3::write(frame, flags);
            }
            TLB_SHOOTDOWN_PENDING.store(false, Ordering::Release);
        }
        IPI_RESCHEDULE => {
            // 触发重新调度：标记 need_sched
            let tcb = mos_task::current_tcb();
            tcb.set_need_schedule();
        }
        _ => {}
    }
}

