use mos_abi::error::{KError, KResult};
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_lib::align::align_up;
use mos_mem::virt::{BackingStore, MapFile, MemoryFlags};
use mos_task::current_tcb;
use vfs_common::OpenFlags;
use x86_64::VirtAddr;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;

pub fn sys_mmap(
    start: Option<Page>,
    pages: u64,
    prot: MemoryProtFlags,
    flags: MemoryMapFlags,
    fd: i32,
    offset: u64,
) -> KResult<PageRange> {
    let mem_flags = MemoryFlags::new(prot, flags);
    let current_tcb = current_tcb();
    let backing = if !flags.contains(MemoryMapFlags::ANONYMOUS) {
        if fd < 0 {
            return Err(KError::NoFile);
        }
        let file = current_tcb
            .process
            .files
            .lock()
            .get(FileDesc(fd as _))
            .cloned()
            .ok_or(KError::NoFile)?;
        let open_flags = file.flags();

        if prot.contains(MemoryProtFlags::READ) && !open_flags.contains(OpenFlags::READ) {
            return Err(KError::NoPermission);
        }

        if prot.contains(MemoryProtFlags::WRITE)
            && flags.contains(MemoryMapFlags::SHARED)
            && !open_flags.contains(OpenFlags::WRITE)
        {
            return Err(KError::NoPermission);
        }

        BackingStore::File(MapFile::new(file.inode().clone(), offset))
    } else {
        // ignore fd and offset
        BackingStore::Anonymous
    };
    current_tcb
        .process
        .mem
        .mmap(start, pages, mem_flags, backing)
}

pub fn sys_munmap(start: Page, pages: u64) -> KResult<()> {
    current_tcb().process.mem.munmap(start, pages)
}

/// mprotect(2): addr must be page-aligned (EINVAL), len rounds up to pages.
/// prot bits are Linux PROT_READ/PROT_WRITE/PROT_EXEC, same values as
/// [`MemoryProtFlags`]; unknown bits are ignored.
pub fn sys_mprotect(addr: u64, len: u64, prot_bits: u64) -> KResult<()> {
    if addr % 4096 != 0 || len == 0 {
        return Err(KError::InvalidArg);
    }
    let start = Page::containing_address(VirtAddr::new(addr));
    let pages = align_up(len, 4096) / 4096;
    let prot = MemoryProtFlags::from_bits_truncate(prot_bits as u32);
    current_tcb().process.mem.protect(start, pages, prot)
}

/// madvise(2): accept-and-ignore — MOS has no page cache or swap, so all
/// advice is trivially satisfied.
pub fn sys_madvise(_addr: u64, _len: u64, _advice: u64) -> KResult<()> {
    Ok(())
}
