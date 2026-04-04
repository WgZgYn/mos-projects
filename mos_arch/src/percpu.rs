use crate::sync::IrqLocked;
use core::mem::offset_of;
use lazy_static::lazy_static;

#[repr(C)]
pub struct PerCpu {
    pub kernel_stack_top: *mut u64,
    pub temp_rsp: *mut u64,
}

impl Default for PerCpu {
    fn default() -> Self {
        Self::new()
    }
}

impl PerCpu {
    pub fn new() -> Self {
        Self {
            kernel_stack_top: core::ptr::null_mut(),
            temp_rsp: core::ptr::null_mut(),
        }
    }
}

unsafe impl Sync for PerCpu {}
unsafe impl Send for PerCpu {}

lazy_static! {
    pub static ref PER_CPU: IrqLocked<PerCpu> = unsafe { IrqLocked::new(PerCpu::new()) };
}

pub const KERNEL_STACK_OFFSET: usize = offset_of!(PerCpu, kernel_stack_top);
pub const TEMP_RSP_OFFSET: usize = offset_of!(PerCpu, temp_rsp);

pub fn update_per_cpu(f: impl FnOnce(&mut PerCpu)) {
    PER_CPU.access(f)
}
