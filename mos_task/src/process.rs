use mos_abi::error::{KError, KResult};
use mos_arch::sync::SpinIrqLock;
use crate::thread::{Priority, ThreadControlBlock, Tid};
use crate::wait_queue::WaitQueue;
use alloc::boxed::Box;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::mem::MaybeUninit;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use lazy_static::lazy_static;
use mos_abi::fs::FileDesc;
use mos_abi::memory::{MemoryMapFlags, MemoryProtFlags};
use mos_arch::structs::CpuContext;
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
use x86_64::structures::paging::Page;
use x86_64::structures::paging::page::PageRange;

pub const DEFAULT_USER_STACK_PAGES: u64 = 4; // currently only 16 KiB
pub const DEFAULT_KERNEL_STACK_PAGES: usize = 8;

lazy_static! {
    pub static ref KERNEL: KRef<ProcessControlBlock> = ProcessControlBlock::kernel();
}

// use slab as the allocator
static PCB_SLAB: SpinIrqLock<Slab<ProcessControlBlock>> = SpinIrqLock::new(Slab::new());
// PID identity outlives the PCB slot (the parent may still own a zombie).
// Do not wrap: exhaustion returns EAGAIN until a reusable PID allocator exists.
static NEXT_PID: AtomicU64 = AtomicU64::new(1);

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
        let _rsp = unsafe { data.as_ptr().add(page_size) };
        // info!("stack low: {:?}, top: {:?}", data.as_ptr(), rsp);
        Self { _data: data }
    }
    pub fn top(&self) -> VirtAddr {
        unsafe { VirtAddr::from_ptr(self._data.as_ptr().add(self._data.len())) }
    }
}
impl Default for KernelStack {
    fn default() -> Self {
        Self::new(DEFAULT_KERNEL_STACK_PAGES)
    }
}

pub trait ImageResource: Send + Sync {
    fn key(&self) -> (&'static str, u64);
}

pub struct SharedState {
    pub child_pid: Pid,
    pub exited: AtomicBool,
    pub wait_status: AtomicI32,
    pub(crate) pgid: AtomicU64,
    pub(crate) sid: AtomicU64,
    pub(crate) executed: AtomicBool,
}

impl SharedState {
    fn new_shared(child_pid: Pid) -> Arc<Self> {
        Arc::new(Self {
            child_pid,
            exited: AtomicBool::new(false),
            wait_status: AtomicI32::new(0),
            pgid: AtomicU64::new(0),
            sid: AtomicU64::new(0),
            executed: AtomicBool::new(false),
        })
    }
}

#[derive(Clone)]
pub struct FileTable {
    table: Vec<Option<Arc<File>>>,
}

impl FileTable {
    pub const fn empty() -> Self {
        Self { table: Vec::new() }
    }
    pub fn insert(&mut self, file: File) -> FileDesc {
        self.insert_arc(Arc::new(file))
    }
    pub fn insert_arc(&mut self, file: Arc<File>) -> FileDesc {
        // reuse the first hole, else append
        if let Some((i, slot)) = self.table.iter_mut().enumerate().find(|(_, s)| s.is_none()) {
            *slot = Some(file);
            FileDesc(i)
        } else {
            self.table.push(Some(file));
            FileDesc(self.table.len() - 1)
        }
    }
    /// insert at a specific index (dup2/dup3/F_DUPFD), replacing any file
    /// that was there (its Drop runs on_close)
    pub fn insert_at(&mut self, index: usize, file: Arc<File>) -> FileDesc {
        if index >= self.table.len() {
            self.table.resize_with(index + 1, || None);
        }
        self.table[index] = Some(file);
        FileDesc(index)
    }
    pub fn remove(&mut self, file: FileDesc) -> bool {
        if file.0 < self.table.len() && self.table[file.0].is_some() {
            self.table[file.0] = None;
            true
        } else {
            false
        }
    }
    pub fn get(&self, file: FileDesc) -> Option<&Arc<File>> {
        self.table.get(file.0).and_then(|s| s.as_ref())
    }
    pub fn dup(&mut self, file: FileDesc) -> Option<FileDesc> {
        self.get(file).cloned().map(|f| self.insert_arc(f))
    }
    /// dup2/dup3 semantics: close dst if it was open, then point it at src
    pub fn dup_to(&mut self, src: FileDesc, dst: FileDesc) -> bool {
        match self.get(src).cloned() {
            Some(f) => {
                self.insert_at(dst.0, f);
                true
            }
            None => false,
        }
    }
    /// F_DUPFD: duplicate to the lowest free fd >= min
    pub fn dup_from(&mut self, src: FileDesc, min: usize) -> Option<FileDesc> {
        let file = self.get(src)?.clone();
        let mut index = min;
        loop {
            if index >= self.table.len() {
                return Some(self.insert_at(index, file));
            }
            if self.table[index].is_none() {
                return Some(self.insert_at(index, file));
            }
            index += 1;
        }
    }
    pub fn len(&self) -> usize {
        self.table.iter().filter(|s| s.is_some()).count()
    }
    pub fn is_empty(&self) -> bool {
        self.table.iter().all(|s| s.is_none())
    }
}

pub struct ProcessControlBlock {
    slab_key: usize,
    /// unique process id
    pub pid: Pid,

    /// pid of the parent process; 0 for the init process
    pub ppid: AtomicU64,

    /// determine the process running ring level
    pub context: CpuContext,

    /// the address space the process uses
    pub mem: MemorySet,

    /// manager threads
    pub(crate) threads: Mutex<Vec<Tid>>,

    refcount: AtomicUsize,

    /// child process exit state shared with parent
    pub childs: Mutex<Vec<Arc<SharedState>>>,

    /// All child exits notify one queue, so wait4(-1) need not select a child
    /// in advance. Arc keeps the queue alive if the parent exits first.
    pub child_exit_queue: Arc<WaitQueue>,
    pub parent_exit_queue: Mutex<Option<Arc<WaitQueue>>>,

    /// this is self exit state shared with parent
    pub shared: Arc<SharedState>,

    /// open file table
    pub files: Mutex<FileTable>,

    /// Resources tied to this executable image (for example display handles).
    /// Fork does not transfer ownership; exec and exit release them before memory.
    pub image_resources: Mutex<Vec<Box<dyn ImageResource>>>,

    /// current working directory
    pub work_dir: Mutex<Arc<Dentry>>,

    /// umask(2) value for file mode calculation
    pub umask: AtomicU32,

    /// Dispositions are shared by threads; keep the table off the slab entry.
    pub signals: Mutex<Box<crate::signal::ProcessSignals>>,
}

impl ProcessControlBlock {
    pub fn files(&self) -> &Mutex<FileTable> { &self.files }
    pub fn work_dir(&self) -> &Mutex<Arc<Dentry>> { &self.work_dir }
    pub fn shared(&self) -> &Arc<SharedState> { &self.shared }
    pub fn childs(&self) -> &Mutex<Vec<Arc<SharedState>>> { &self.childs }
    pub fn threads(&self) -> &Mutex<Vec<Tid>> { &self.threads }

    /// all the new construction of PCB will insert into PCBTables
    /// will allocate frame for root page table
    fn new_empty(user_space: bool, new_addr_space: bool) -> KResult<KRef<Self>> {
        let pid = Pid(NEXT_PID.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| {
            (n <= i32::MAX as u64).then_some(n + 1)
        }).map_err(|_| KError::StillRunning)?);
        let page_table = if new_addr_space {
            PageTableManager::alloc_page_table()?
        } else {
            PageTableManager::kernel_page_table()
        };

        let context = if user_space {
            CpuContext::user_context()
        } else {
            CpuContext::kernel_context()
        };

        let mut guard = PCB_SLAB.lock();
        let pcb = guard.insert_with_key(|key| Self {
            slab_key: key,
            pid,
            ppid: AtomicU64::new(0),
            mem: MemorySet::new(page_table, user_space),
            threads: Mutex::new(Vec::new()),
            context,
            refcount: AtomicUsize::new(1),
            childs: Mutex::new(Vec::new()),
            child_exit_queue: Arc::new(WaitQueue::new()),
            parent_exit_queue: Mutex::new(None),
            shared: SharedState::new_shared(pid),
            files: Mutex::new(FileTable::empty()),
            image_resources: Mutex::new(Vec::new()),
            work_dir: Mutex::new(vfs_root_dentry()),
            umask: AtomicU32::new(0o022),
            signals: Mutex::new(Box::new(crate::signal::ProcessSignals::default())),
        });
        Ok(unsafe { KRef::from_ptr(NonNull::from_ref(pcb)) })
    }
    pub fn new_empty_user() -> KResult<KRef<Self>> {
        let process = Self::new_empty(true, true)?;
        crate::job::register(&process.shared, None);
        Ok(process)
    }
    /// be careful, this will share the same CR3
    pub fn new_empty_kernel() -> KRef<Self> {
        Self::new_empty(false, false).unwrap()
    }

    /// will return stack top VirtAddr
    pub fn prepare_user_stack(&self) -> KResult<Page> {
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

    pub fn fork(&self) -> KResult<KRef<ProcessControlBlock>> {
        let is_user = self.mem.is_userspace();
        let new_process = ProcessControlBlock::new_empty(is_user, true)?;
        new_process
            .ppid
            .store(self.pid.as_u64(), Ordering::Release);

        // copy the open files and cwd
        *new_process.files.lock() = self.files.lock().clone();
        *new_process.work_dir.lock() = self.work_dir.lock().clone();
        **new_process.signals.lock() = self.signals.lock().fork_child();
        new_process
            .umask
            .store(self.umask.load(Ordering::Acquire), Ordering::Release);

        // Complete fallible setup before publishing a child to its parent.
        // Otherwise a failed fork leaves a child that can never exit, and wait
        // can sleep forever on that abandoned shared state.
        self.mem.share_with(&new_process.mem)?;

        // The child is not runnable until sys_fork enqueues its first thread.
        let shared_state = new_process.shared.clone();
        crate::job::register(&shared_state, Some(&self.shared));
        *new_process.parent_exit_queue.lock() = Some(self.child_exit_queue.clone());
        self.childs.lock().push(shared_state);

        Ok(new_process)
    }
}

impl Drop for ProcessControlBlock {
    fn drop(&mut self) {
        self.image_resources.get_mut().clear();
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
        PCB_SLAB.lock().remove_inplace(self.slab_key);
    }
}
