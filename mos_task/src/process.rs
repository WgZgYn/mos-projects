use crate::thread::{Priority, ThreadControlBlock, Tid};
use crate::wait_queue::WaitQueue;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize};
use data_public::struct_field_public;
use lazy_static::lazy_static;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_arch::structs::ProcessContext;
use mos_arch::sync::SpinIrqLock;
use mos_lib::Frame4KiB;
use mos_lib::sync::{KRef, RefCounted};
use mos_mem::PageTableManager;
use mos_mem::slab::Slab;
use mos_mem::virt::{MemoryArea, MemoryFlags, MemorySet};
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use mos_vfs::vfs_root_dentry;
use spin::Mutex;
use x86_64::VirtAddr;
use x86_64::registers::rflags::RFlags;
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;

pub const DEFAULT_RFLAGS: RFlags = RFlags::INTERRUPT_FLAG; // IF=1
pub const DEFAULT_USER_STACK_PAGES: u64 = 4; // currently only 16 KiB
pub const DEFAULT_KERNEL_STACK_PAGES: usize = 4;

lazy_static! {
    pub static ref KERNEL: KRef<ProcessControlBlock> = ProcessControlBlock::kernel();
}

// use slab as the allocator
static PCB_SLAB: SpinIrqLock<Slab<ProcessControlBlock>> = SpinIrqLock::new(Slab::new());

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Pid(u64);

impl Pid {
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// a bounded memory for kernel stack
/// 4KiB page aligned
pub struct KernelStack {
    _data: Box<[Frame4KiB]>,
}
impl KernelStack {
    /// The `page_size` is the num of 4KiB Frame
    pub fn new(page_size: usize) -> Self {
        let data: Box<[MaybeUninit<Frame4KiB>]> = Box::new_uninit_slice(page_size);
        let data = unsafe { data.assume_init() };
        let rsp = unsafe { data.as_ptr().add(page_size) };
        // info!("stack low: {:?}, top: {:?}", data.as_ptr(), rsp);
        Self { _data: data }
    }
    pub fn top(&self) -> VirtAddr {
        unsafe { VirtAddr::from_ptr(self._data.as_ptr().add(self._data.len())) }
    }
}

pub struct SharedState {
    pub child_pid: Pid,
    pub exited: AtomicBool,
    pub exit_code: AtomicI32,
    pub wait_queue: WaitQueue,
}

impl SharedState {
    fn new_shared(child_pid: Pid) -> Arc<Self> {
        Arc::new(Self {
            child_pid,
            exited: AtomicBool::new(false),
            exit_code: AtomicI32::new(0),
            wait_queue: WaitQueue::new(),
        })
    }
}

#[derive(Clone)]
pub struct FileTable {
    slab: slab::Slab<Arc<File>>,
}

impl FileTable {
    pub const fn empty() -> Self {
        Self {
            slab: slab::Slab::new(),
        }
    }
    pub fn insert(&mut self, file: File) -> FileDesc {
        FileDesc(self.slab.insert(Arc::new(file)))
    }
    pub fn insert_arc(&mut self, file: Arc<File>) -> FileDesc {
        FileDesc(self.slab.insert(file))
    }
    pub fn remove(&mut self, file: FileDesc) -> bool {
        self.slab.try_remove(file.0).is_some()
    }
    pub fn get(&self, file: FileDesc) -> Option<&Arc<File>> {
        self.slab.get(file.0)
    }
    pub fn len(&self) -> usize {
        self.slab.len()
    }
}

struct_field_public! {
    pub struct ProcessControlBlock {
        /// unique process id
        pid: Pid,

        /// determine the process running ring level
        context: ProcessContext,

        /// the address space the process uses
        mem: MemorySet,

        /// manager threads
        threads: Mutex<Vec<Tid>>,

        refcount: AtomicUsize,

        /// this is childs share state
        childs: Mutex<Vec<Arc<SharedState>>>,

        /// this is self share state
        shared: Arc<SharedState>,

        files: Mutex<FileTable>,

        work_dir: Mutex<Arc<Dentry>>,
    }
}

impl ProcessControlBlock {
    /// all the new construction of PCB will insert into PCBTables
    /// will allocate frame for root page table
    fn new_empty(user_space: bool, new_addr_space: bool) -> Result<KRef<Self>, &'static str> {
        let page_table = if new_addr_space {
            PageTableManager::alloc_page_table()?
        } else {
            PageTableManager::kernel_page_table()
        };

        let context = if user_space {
            ProcessContext::user_context()
        } else {
            ProcessContext::kernel_context()
        };

        let pcb = PCB_SLAB.access(|slab| {
            let v = slab.insert_with_key(|key| Self {
                pid: Pid(key as _),
                mem: MemorySet::new(page_table, user_space),
                threads: Mutex::new(Vec::new()),
                context,
                refcount: AtomicUsize::new(1),
                childs: Mutex::new(Vec::new()),
                shared: SharedState::new_shared(Pid(key as _)),
                files: Mutex::new(FileTable::empty()),
                work_dir: Mutex::new(vfs_root_dentry()),
            });
            unsafe { KRef::from_ptr(NonNull::from_ref(v)) }
        });
        Ok(pcb)
    }
    pub fn new_empty_user() -> Result<KRef<Self>, &'static str> {
        Self::new_empty(true, true)
    }
    /// be careful, this will share the same CR3
    pub fn new_empty_kernel() -> KRef<Self> {
        Self::new_empty(false, false).unwrap()
    }

    /// will return stack top VirtAddr
    pub fn prepare_user_stack(&self) -> Result<Page, &'static str> {
        let stack_high = Page::containing_address(VirtAddr::new(0x7FFF_0000_0000));
        let stack_low = stack_high - DEFAULT_USER_STACK_PAGES;

        self.mem.addr_space.access(|mm| {
            mm.try_add_area(MemoryArea::new_anon(
                PageRange {
                    start: stack_low,
                    end: stack_high,
                },
                MemoryFlags::new(MemoryProtFlags::READ_WRITE, MemoryMapFlags::PRIVATE),
            ))
        })?;

        Ok(stack_high)
    }

    pub fn clear_mem(pcb: &KRef<Self>) {
        pcb.mem.clear()
    }

    pub fn load_with_mem<R, F: FnOnce(&MemorySet) -> R>(&self, f: F) -> R {
        f(&self.mem)
    }

    /// Scheduler's first schedule is schedule from idle to other
    /// so we inits the idle with Running state.
    fn kernel() -> KRef<ProcessControlBlock> {
        lazy_static! {
            static ref KERNEL: KRef<ProcessControlBlock> = ProcessControlBlock::new_empty_kernel();
        }
        KERNEL.clone()
    }

    pub fn kernel_func(
        pcb: KRef<Self>,
        f: fn() -> !,
        priority: Priority,
    ) -> KRef<ThreadControlBlock> {
        let kstack = KernelStack::new(DEFAULT_KERNEL_STACK_PAGES);
        ThreadControlBlock::create(
            pcb,
            VirtAddr::new(f as *const () as _),
            None,
            kstack,
            priority,
        )
    }

    /// actually it's unnecessary to clean every time when just one thread exit
    pub fn clean_exit_thread(&self, tid: Tid) {
        let mut threads = self.threads.lock();
        let index = threads
            .iter()
            .enumerate()
            .find(|(_, t)| **t == tid)
            .map(|(i, _)| i)
            .expect("should exist");
        threads.swap_remove(index);
    }

    pub fn fork(&self) -> Result<KRef<ProcessControlBlock>, &'static str> {
        let is_user = self.mem.is_userspace();
        let new_process = ProcessControlBlock::new_empty(is_user, true)?;

        // copy the open files and cwd
        *new_process.files.lock() = self.files.lock().clone();
        *new_process.work_dir.lock() = self.work_dir.lock().clone();

        // register the shared state
        let shared_state = new_process.shared.clone();
        self.childs.lock().push(shared_state);

        // share the memset
        self.mem.share_with(&new_process.mem)?; // this should kill the process

        Ok(new_process)
    }
}

impl Drop for ProcessControlBlock {
    fn drop(&mut self) {
        if self.mem.is_userspace() {
            let frame = self.mem.page_table_frame();
            PageTableManager::free_cr3(frame);
        }
    }
}

impl RefCounted for ProcessControlBlock {
    fn ref_count(&self) -> &AtomicUsize {
        &self.refcount
    }
    unsafe fn destroy(&self) {
        PCB_SLAB.access(|slab| slab.remove_inplace(self.pid.0 as _));
    }
}
