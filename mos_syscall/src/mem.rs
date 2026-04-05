use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_mem::virt::{BackingStore, MapFile, MemoryFlags};
use mos_task::current_tcb;
use vfs_common::OpenFlags;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;

pub fn sys_mmap(
    start: Option<Page>,
    pages: u64,
    prot: MemoryProtFlags,
    flags: MemoryMapFlags,
    fd: i32,
    offset: u64,
) -> Result<PageRange, &'static str> {
    let mem_flags = MemoryFlags::new(prot, flags);
    let current_tcb = current_tcb();
    let backing = if !flags.contains(MemoryMapFlags::ANONYMOUS) {
        if fd < 0 {
            return Err("need file descriptor");
        }
        let file = current_tcb
            .process
            .files
            .lock()
            .get(FileDesc(fd as _))
            .cloned()
            .ok_or("no such file")?;
        let open_flags = file.flags();

        if prot.contains(MemoryProtFlags::READ) && !open_flags.contains(OpenFlags::READ) {
            return Err("permission reading deny");
        }

        if prot.contains(MemoryProtFlags::WRITE)
            && flags.contains(MemoryMapFlags::SHARED)
            && !open_flags.contains(OpenFlags::WRITE)
        {
            return Err("permission writing deny");
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

pub fn sys_munmap(start: Page, pages: u64) -> Result<(), &'static str> {
    current_tcb().process.mem.munmap(start, pages)
}
