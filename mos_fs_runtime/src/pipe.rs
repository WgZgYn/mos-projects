use alloc::sync::Arc;
use collections::array::ringbuf::bounded::RingBuffer;
use mos_task::wait_queue::WaitQueue;
use mos_task::{current_tcb, schedule};
use spin::Mutex;
use vfs_common::{FileType, IoContext, OpenFlags, VfsError, VfsResult};
use vfs_traits::{Inode, StreamOps};

struct PipeInner {
    readers: usize,
    writers: usize,
    buffer: RingBuffer<u8, 512>,
    read_queue: WaitQueue,
    write_queue: WaitQueue,
}

impl PipeInner {
    const fn new() -> Self {
        Self {
            readers: 0,
            writers: 0,
            buffer: RingBuffer::new(),
            read_queue: WaitQueue::new(),
            write_queue: WaitQueue::new(),
        }
    }
}

pub struct PipeInode {
    inner: Mutex<PipeInner>,
}

impl Inode for PipeInode {
    fn file_type(&self) -> FileType {
        FileType::Fifo
    }
    fn as_stream_ops(&self) -> VfsResult<&dyn StreamOps> {
        Ok(self)
    }
    fn on_open(&self, flags: OpenFlags) -> VfsResult<()> {
        let mut inner = self.inner.lock();
        if flags.contains(OpenFlags::READ) {
            inner.readers += 1;
        }
        if flags.contains(OpenFlags::WRITE) {
            inner.writers += 1;
        }
        Ok(())
    }
    fn on_close(&self, flags: OpenFlags) -> VfsResult<()> {
        let mut inner = self.inner.lock();
        if flags.contains(OpenFlags::READ) {
            inner.readers -= 1;
            inner.write_queue.wake_all();
        }
        if flags.contains(OpenFlags::WRITE) {
            inner.writers -= 1;
            inner.read_queue.wake_all();
        }
        Ok(())
    }
}
impl StreamOps for PipeInode {
    fn read(&self, ctx: &IoContext, buf: &mut [u8]) -> VfsResult<usize> {
        loop {
            let mut inner = self.inner.lock();

            if !inner.buffer.is_empty() {
                let n = inner.buffer.read_slice(buf);
                inner.write_queue.wake_all();
                return Ok(n);
            }

            if inner.writers == 0 {
                return Ok(0);
            }

            if ctx.nonblock {
                return Err(VfsError::Pendding);
            }

            let current_tcb = current_tcb();
            inner.read_queue.wait(current_tcb);
            drop(inner);
            schedule();
            // we need recheck to prevent wake from other's open
        }
    }

    fn write(&self, ctx: &IoContext, buf: &[u8]) -> VfsResult<usize> {
        loop {
            let mut inner = self.inner.lock();

            if inner.readers == 0 {
                return Err(VfsError::BrokenPipe);
            }

            if !inner.buffer.is_full() {
                let n = inner.buffer.write_slice(buf);
                inner.read_queue.wake_all();
                return Ok(n);
            }

            if ctx.nonblock {
                return Err(VfsError::Pendding);
            }

            let current_tcb = current_tcb();
            inner.write_queue.wait(current_tcb);
            // this is necessary to release the lock to prevent deadlock
            drop(inner);
            schedule();
            // we need recheck to prevent wake from other's open
        }
    }
}

impl PipeInode {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(PipeInner::new()),
        }
    }
}

/// this will not inc the readers and writers count
pub fn make_pipe() -> Arc<PipeInode> {
    Arc::new(PipeInode {
        inner: Mutex::new(PipeInner::new()),
    })
}

pub struct FifoInode {
    pipe: PipeInode, // Mutex<PipeInner>
}

impl Inode for FifoInode {
    fn file_type(&self) -> FileType {
        FileType::Fifo
    }
    fn as_stream_ops(&self) -> VfsResult<&dyn StreamOps> {
        self.pipe.as_stream_ops()
    }
    fn on_open(&self, flags: OpenFlags) -> VfsResult<()> {
        let is_writer = flags.contains(OpenFlags::WRITE);
        let is_reader = flags.contains(OpenFlags::READ);
        self.pipe.on_open(flags)?; // here inc the reader, writes count
        match (is_reader, is_writer) {
            // NonBlock, wake all
            (true, true) => {
                let inner = self.pipe.inner.lock();
                inner.write_queue.wake_all();
                inner.read_queue.wake_all();
                Ok(())
            }
            //
            (true, _) => loop {
                let inner = self.pipe.inner.lock();
                inner.write_queue.wake_all();
                if inner.writers > 0 {
                    return Ok(());
                }
                inner.read_queue.wait(current_tcb());
                drop(inner);
                schedule();
            },
            (_, true) => loop {
                let inner = self.pipe.inner.lock();
                inner.read_queue.wake_all();
                if inner.readers > 0 {
                    return Ok(());
                }
                inner.write_queue.wait(current_tcb());
                drop(inner);
                schedule();
            },
            (false, false) => Err(VfsError::NotOpen),
        }
    }
    fn on_close(&self, flags: OpenFlags) -> VfsResult<()> {
        self.pipe.on_close(flags)
    }
}

// this will not inc the readers and writers count
pub fn make_fifo_inode() -> Arc<FifoInode> {
    Arc::new(FifoInode {
        pipe: PipeInode::new(),
    })
}
