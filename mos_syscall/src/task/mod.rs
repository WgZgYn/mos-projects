use mos_abi::error::{KError, KResult};
use goblin::elf::program_header::{PF_R, PF_W, PF_X};
use mos_abi::memory::MemoryProtFlags;
use mos_lib::sync::KRef;
use mos_task::process::{DEFAULT_KERNEL_STACK_PAGES, KernelStack, ProcessControlBlock};
use mos_task::thread::{DEFAULT_THREAD_PRIORITY, Priority, ThreadControlBlock};
use mos_vfs::file::File;
use x86_64::VirtAddr;
use crate::task::elf::load_elf;

mod elf;

/// Everything execve needs after loading an ELF: the entry point plus the
/// program-header info for the auxv (AT_PHDR/AT_PHENT/AT_PHNUM).
pub struct ExecLoadInfo {
    pub entry: VirtAddr,
    pub phdr: u64,
    pub phent: usize,
    pub phnum: usize,
}

pub fn load_elf_from_file(
    pcb: &KRef<ProcessControlBlock>,
    file: &File,
) -> KResult<ExecLoadInfo> {
    if !pcb.mem.is_userspace() {
        return Err(KError::Unsupported);
    }
    let (elf, _libs) = load_elf(&pcb.mem, file, true)?;
    let (phdr, phent, phnum) = elf.phdr_info();
    Ok(ExecLoadInfo {
        entry: elf.entry(),
        phdr,
        phent,
        phnum,
    })
}

pub fn pure_load_elf(
    pcb: &KRef<ProcessControlBlock>,
    elf_bs: &[u8],
) -> KResult<VirtAddr> {
    pub fn flags_elf_to_prot(elf_flags: u32) -> MemoryProtFlags {
        let mut flags = MemoryProtFlags::empty();
        if elf_flags & PF_R != 0 {
            flags.insert(MemoryProtFlags::READ);
        }
        if elf_flags & PF_W != 0 {
            flags.insert(MemoryProtFlags::WRITE);
        }
        if elf_flags & PF_X != 0 {
            flags.insert(MemoryProtFlags::EXEC);
        }
        flags
    }

    if !pcb.mem.is_userspace() {
        return Err(KError::Unsupported);
    }
    let elf = goblin::elf::Elf::parse(elf_bs).map_err(|_| "ELF parse error")?;
    let entry = VirtAddr::new(elf.entry);
    for ph in elf.program_headers {
        if ph.p_type != goblin::elf::program_header::PT_LOAD {
            continue;
        }
        pcb.mem.load_data(
            VirtAddr::new(ph.p_vaddr),
            ph.p_memsz,
            &elf_bs[ph.p_offset as usize..(ph.p_offset + ph.p_filesz) as usize],
            flags_elf_to_prot(ph.p_flags),
        )?;
    }
    Ok(entry)
}

pub fn spawn_thread(
    pcb: KRef<ProcessControlBlock>,
    elf_bs: &[u8],
    priority: Option<Priority>,
) -> KResult<KRef<ThreadControlBlock>> {
    let entry = pure_load_elf(&pcb, elf_bs)?;

    // ensure the stack align requirement `rsp % 16 == 8`
    // this will be done at user crt0
    let stack = pcb.prepare_user_stack()?.start_address() - 64;

    Ok(ThreadControlBlock::create(
        pcb,
        entry,
        Some(stack),
        KernelStack::new(DEFAULT_KERNEL_STACK_PAGES),
        priority.unwrap_or(Priority(DEFAULT_THREAD_PRIORITY)),
    ))
}
