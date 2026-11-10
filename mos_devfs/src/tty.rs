use crate::{DEV_DIR_DENTRY, DEV_DIR_INODE};
use alloc::sync::Arc;
use lazy_static::lazy_static;
use mos_driver::keyboard::CharSyncStream;
use mos_vfs::dentry::Dentry;
use mos_vfs::file::File;
use vfs_common::{FileType, IoContext, OpenFlags, VfsError, VfsResult};
use vfs_traits::*;

pub struct TtyInode;

struct TtyInput {
    pending: [u8; 4096],
    head: usize,
    length: usize,
    /// Bytes that may be returned now. In canonical mode an unfinished line
    /// remains after this prefix; in raw mode this equals `length`.
    readable: usize,
    eof: bool,
    termios: mos_abi::fs::Termios,
    readers: mos_task::wait_queue::WaitQueueInner,
}

impl TtyInput {
    fn push(&mut self, bytes: &[u8]) -> bool {
        if bytes.len() > self.pending.len() - self.length { return false; }
        for byte in bytes {
            let tail = (self.head + self.length) % self.pending.len();
            self.pending[tail] = *byte;
            self.length += 1;
        }
        true
    }

    fn erase_uncommitted_character(&mut self) -> bool {
        if self.length == self.readable { return false; }
        while self.length > self.readable {
            self.length -= 1;
            let index = (self.head + self.length) % self.pending.len();
            if self.pending[index] & 0xc0 != 0x80 { break; }
        }
        true
    }

    fn canonical_prefix(&self, limit: usize) -> usize {
        let mut count = limit.min(self.readable);
        for index in 0..count {
            if self.pending[(self.head + index) % self.pending.len()] == b'\n' {
                count = index + 1;
                break;
            }
        }
        count
    }

    fn recompute_readable(&mut self) {
        if self.termios.c_lflag & mos_abi::fs::ICANON == 0 {
            self.readable = self.length;
            return;
        }
        self.readable = 0;
        for index in 0..self.length {
            if self.pending[(self.head + index) % self.pending.len()] == b'\n' {
                self.readable = index + 1;
            }
        }
    }
}

lazy_static! {
    static ref TTY_INPUT: spin::Mutex<TtyInput> = spin::Mutex::new(TtyInput {
        pending: [0; 4096], head: 0, length: 0, readable: 0, eof: false,
        termios: mos_abi::fs::default_termios(),
        readers: mos_task::wait_queue::WaitQueueInner::new(),
    });
    pub static ref TTY_INODE: Arc<TtyInode> = Arc::new(TtyInode);
    pub static ref TTY_DENTRY: Arc<Dentry> = DEV_DIR_DENTRY.lookup("tty").unwrap();
    pub static ref FSTDIN: Arc<File> =
        Arc::new(File::from_dentry(TTY_DENTRY.clone(), 0, OpenFlags::READ).unwrap());
    pub static ref FSTDOUT: Arc<File> =
        Arc::new(File::from_dentry(TTY_DENTRY.clone(), 0, OpenFlags::WRITE).unwrap());
}

/// One permanent consumer owns keyboard decoding. Reading a terminal never
/// owns a decoder or holds the input mutex while waiting for a key.
pub fn start_input_worker() {
    use core::sync::atomic::{AtomicBool, Ordering};
    static STARTED: AtomicBool = AtomicBool::new(false);
    if STARTED.swap(true, Ordering::AcqRel) { return; }
    mos_driver::framebuffer::enable_terminal();
    mos_driver::keyboard::try_init_once_scancode_queue();
    lazy_static::initialize(&TTY_INPUT);
    mos_task::enqueue_task(mos_task::process::ProcessControlBlock::kernel_func(
        mos_task::process::KERNEL.clone(), input_worker, mos_task::thread::Priority(2)));
}

fn input_worker() -> ! {
    let mut stream = CharSyncStream::default();
    loop {
        let mut character = stream.next_char();
        let mut encoded = [0; 4];
        let mut input = TTY_INPUT.lock();
        if character == '\r' && input.termios.c_iflag & mos_abi::fs::ICRNL != 0 {
            character = '\n';
        }
        let bytes = character.encode_utf8(&mut encoded).as_bytes();
        let control = (character as u32 <= u8::MAX as u32).then_some(character as u8);
        let canonical = input.termios.c_lflag & mos_abi::fs::ICANON != 0;
        let echo = input.termios.c_lflag & mos_abi::fs::ECHO != 0;
        let output_flags = input.termios.c_oflag;

        if input.termios.c_lflag & mos_abi::fs::ISIG != 0
            && control == Some(input.termios.c_cc[mos_abi::fs::VINTR]) {
            if input.termios.c_lflag & mos_abi::fs::NOFLSH == 0 {
                input.head = 0;
                input.length = 0;
                input.readable = 0;
                input.eof = false;
            }
            drop(input);
            if echo { write_output(b"^C\n",output_flags); }
            if let Some(group) = mos_task::job::terminal_foreground_for_input() {
                for target in mos_task::job::signal_targets(group) {
                    mos_task::signal::publish_standard(&target, mos_abi::signal::SIGINT, 0, 128);
                }
            }
            continue;
        }

        let mut echo_erase = false;
        let mut echo_kill = false;
        let wake = if canonical && control == Some(input.termios.c_cc[mos_abi::fs::VERASE]) {
            echo_erase = input.erase_uncommitted_character();
            false
        } else if canonical && control == Some(input.termios.c_cc[mos_abi::fs::VKILL]) {
            if input.length != input.readable {
                input.length = input.readable;
                echo_kill = true;
            }
            false
        } else if canonical && control == Some(input.termios.c_cc[mos_abi::fs::VEOF]) {
            if input.length == input.readable { input.eof = true; }
            else { input.readable = input.length; }
            true
        } else if input.push(bytes) {
            if canonical {
                if character == '\n' { input.readable = input.length; true } else { false }
            } else {
                input.readable = input.length;
                let minimum = input.termios.c_cc[mos_abi::fs::VMIN] as usize;
                input.readable >= minimum.max(1)
            }
        } else { false };

        if wake {
            // Keep resource ownership through normal wakeup, so cancellation
            // cannot leave an old reader wake racing its next unrelated wait.
            mos_task::scheduler::wake_blocked_task(input.readers.wake_all());
        }
        drop(input);
        if echo_erase && echo { write_output(b"\x08 \x08",output_flags); }
        else if echo_kill && echo { write_output(b"\n",output_flags); }
        else if echo && !(canonical && matches!(control, Some(4) | Some(8) | Some(21))) {
            write_output(bytes,output_flags);
        }
    }
}

impl FileOps for TtyInode {
    fn read(&self, ctx: &IoContext, offset: usize, buf: &mut [u8]) -> VfsResult<usize> {
        self.read_to(ctx, offset, buf.len(), &mut |bytes| {
            buf[..bytes.len()].copy_from_slice(bytes);
            Ok(())
        })
    }

    fn read_to(&self, ctx: &IoContext, _offset: usize, limit: usize,
        output: &mut dyn FnMut(&[u8]) -> VfsResult<()>) -> VfsResult<usize> {
        if limit == 0 { return Ok(0); }
        loop {
            let mut input = TTY_INPUT.lock();
            let canonical = input.termios.c_lflag & mos_abi::fs::ICANON != 0;
            let available = if canonical { input.canonical_prefix(limit) }
                else { limit.min(input.readable) };
            if available != 0 {
                let mut buffer = [0; 256];
                let n = available.min(buffer.len());
                for (index, byte) in buffer[..n].iter_mut().enumerate() {
                    *byte = input.pending[(input.head + index) % input.pending.len()];
                }
                // Commit consumption only after the checked copy succeeds.
                output(&buffer[..n])?;
                input.head = (input.head + n) % input.pending.len();
                input.length -= n;
                input.readable -= n;
                return Ok(n);
            }
            if ctx.nonblock { return Err(VfsError::Pendding); }
            if input.eof {
                input.eof = false;
                return Ok(0);
            }
            if !canonical && input.termios.c_cc[mos_abi::fs::VMIN] == 0 {
                return Ok(0);
            }
            let parked = mos_task::signal::park_interruptible(|task| {
                input.readers.wait_task(task.clone());
                mos_task::scheduler::ParkDecision::Park(input)
            });
            TTY_INPUT.lock().readers.remove_task(mos_task::current_tcb().tid);
            if !parked { return Err(VfsError::Interrupted); }
        }
    }
    fn write(&self, _ctx: &IoContext, _offset: usize, buf: &[u8]) -> VfsResult<usize> {
        let flags = TTY_INPUT.lock().termios.c_oflag;
        write_output(buf,flags);
        Ok(buf.len())
    }
}

impl Inode for TtyInode {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn meta_data(&self) -> &[u8] {
        b"tty0"
    }
    fn file_type(&self) -> FileType {
        FileType::CharDevice
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(Some(DEV_DIR_INODE.clone()))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        Ok(self)
    }
    fn ioctl(&self, cmd: u32, arg: u64, input_bytes: &[u8], output: &mut [u8])
        -> VfsResult<usize> {
        use mos_abi::fs::{
            TCGETS, TCSETS, TCSETSF, TCSETSW, TIOCGPGRP, TIOCGSID, TIOCGWINSZ,
            TIOCSPGRP, TIOCSCTTY, Termios, VTIME,
        };
        match cmd {
            TCGETS => {
                output.copy_from_slice(&TTY_INPUT.lock().termios.to_bytes());
                Ok(0)
            }
            TCSETS | TCSETSW | TCSETSF => {
                let termios = Termios::from_bytes(input_bytes.try_into()
                    .map_err(|_| VfsError::FileNameIllegal)?);
                // Timed VMIN/VTIME reads require a terminal timer race that is
                // not implemented yet; rejecting it is safer than fake success.
                if termios.c_line != 0 || termios.c_cc[VTIME] != 0 {
                    return Err(VfsError::NotSupported);
                }
                let mut input = TTY_INPUT.lock();
                // Writes are synchronous today, so TCSETSW has no output to
                // drain. TCSETSF additionally discards unread type-ahead.
                if cmd == TCSETSF {
                    input.head = 0;
                    input.length = 0;
                    input.readable = 0;
                    input.eof = false;
                }
                input.termios = termios;
                input.recompute_readable();
                let wake = input.readable != 0
                    || (termios.c_lflag & mos_abi::fs::ICANON == 0
                        && termios.c_cc[mos_abi::fs::VMIN] == 0);
                let waiters: alloc::vec::Vec<_> = if wake {
                    input.readers.wake_all().into_iter().collect()
                } else { alloc::vec::Vec::new() };
                drop(input);
                mos_task::scheduler::wake_blocked_task(waiters);
                Ok(0)
            }
            TIOCGWINSZ => {
                let (cols,rows) = mos_driver::framebuffer::terminal_size();
                output[0..2].copy_from_slice(&rows.to_ne_bytes());
                output[2..4].copy_from_slice(&cols.to_ne_bytes());
                output[4..].fill(0);
                Ok(0)
            }
            TIOCSCTTY => {
                mos_task::job::attach_terminal(&mos_task::current_tcb().process.shared, arg != 0)
                    .map_err(job_error)?;
                Ok(0)
            }
            TIOCGPGRP => {
                let group = mos_task::job::terminal_foreground(
                    &mos_task::current_tcb().process.shared).map_err(job_error)?;
                output.copy_from_slice(&(group as i32).to_ne_bytes());
                Ok(0)
            }
            TIOCSPGRP => {
                let group = i32::from_ne_bytes(input_bytes.try_into()
                    .map_err(|_| VfsError::FileNameIllegal)?);
                mos_task::job::set_terminal_foreground(
                    &mos_task::current_tcb().process.shared, group).map_err(job_error)?;
                Ok(0)
            }
            TIOCGSID => {
                let session = mos_task::job::terminal_session(
                    &mos_task::current_tcb().process.shared).map_err(job_error)?;
                output.copy_from_slice(&(session as i32).to_ne_bytes());
                Ok(0)
            }
            _ => Err(VfsError::NotSupported),
        }
    }
}

fn job_error(error: mos_abi::error::KError) -> VfsError {
    use mos_abi::error::KError;
    match error {
        KError::NotTty => VfsError::NotTty,
        KError::InvalidArg => VfsError::FileNameIllegal,
        KError::OperationNotPermitted => VfsError::OperationNotPermitted,
        _ => VfsError::InternelError,
    }
}

pub const STDIN: usize = 0;
pub const STDOUT: usize = 1;
pub const STDERR: usize = 2;

#[derive(Copy, Clone, Debug)]
pub struct Stdin;
#[derive(Copy, Clone, Debug)]
pub struct Stdout;
#[derive(Copy, Clone, Debug)]
pub struct Stderr;

fn write_output(bytes: &[u8], flags: u32) {
    let onlcr = flags & (mos_abi::fs::OPOST | mos_abi::fs::ONLCR)
        == (mos_abi::fs::OPOST | mos_abi::fs::ONLCR);
    mos_display::write_terminal(bytes,onlcr);
    mos_driver::serial::write_bytes(bytes,onlcr);
}
