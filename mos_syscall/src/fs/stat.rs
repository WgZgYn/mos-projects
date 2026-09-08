use alloc::sync::Arc;
use mos_abi::ffi::c_str_to_str;
use mos_abi::fs::{
    DIR_PERM, DT_BLK, DT_CHR, DT_DIR, DT_FIFO, DT_REG, FILE_PERM, FileDesc, S_IFBLK, S_IFCHR,
    S_IFDIR, S_IFIFO, S_IFREG, Stat,
};
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use vfs_common::{FileType, OpenFlags, VfsError, VfsResult, vfs_error_to_errno};

pub fn file_type_to_dt(ft: FileType) -> u8 {
    match ft {
        FileType::Directory => DT_DIR,
        FileType::Regular => DT_REG,
        FileType::BlockDevice => DT_BLK,
        FileType::CharDevice => DT_CHR,
        FileType::Fifo => DT_FIFO,
    }
}

/// Build a Linux-layout struct stat from an open file.
/// Timestamps are filled from the kernel wall clock once available (task 4).
pub fn stat_from_file(file: &mos_vfs::file::File) -> VfsResult<Stat> {
    let ft = file.dentry().file_type()?;
    let (mode_bits, perm) = match ft {
        FileType::Directory => (S_IFDIR, DIR_PERM),
        FileType::Regular => (S_IFREG, FILE_PERM),
        FileType::CharDevice => (S_IFCHR, FILE_PERM),
        FileType::BlockDevice => (S_IFBLK, FILE_PERM),
        FileType::Fifo => (S_IFIFO, FILE_PERM),
    };
    let size = file.dentry().file_size()? as i64;
    // no persistent inode numbers yet: the address of the inode object is
    // stable for the lifetime of the file and unique enough
    let ino = Arc::as_ptr(file.inode()) as *const () as u64;

    let (sec, _) = crate::time::wall_clock();
    Ok(Stat {
        st_dev: 0,
        st_ino: ino,
        st_nlink: 1,
        st_mode: mode_bits | perm,
        st_uid: 0,
        st_gid: 0,
        __pad0: 0,
        st_rdev: 0,
        st_size: size,
        st_blksize: 4096,
        st_blocks: if size > 0 { (size + 511) / 512 } else { 0 },
        st_atime: sec,
        st_mtime: sec,
        st_ctime: sec,
        ..Default::default()
    })
}

pub fn raw_sys_fstat(context: &mut SyscallContext, args: [u64; 6]) {
    let fd = FileDesc(args[0] as usize);
    let ptr = args[1] as *mut Stat;
    context.scratch.rax = match sys_fstat(fd) {
        Ok(stat) => {
            if !ptr.is_null() {
                unsafe { ptr.write_volatile(stat) };
                0
            } else {
                (-vfs_error_to_errno(VfsError::FileNameIllegal)) as u64
            }
        }
        Err(e) => (-vfs_error_to_errno(e)) as u64,
    };
}

pub fn sys_fstat(fd: FileDesc) -> VfsResult<Stat> {
    let current_tcb = current_tcb();
    let file = current_tcb
        .process
        .files()
        .lock()
        .get(fd)
        .cloned()
        .ok_or(VfsError::BadFileDesc)?;
    stat_from_file(&file)
}

pub fn raw_sys_stat(context: &mut SyscallContext, args: [u64; 6]) {
    let ptr = args[1] as *mut Stat;
    context.scratch.rax = match sys_stat_path(args[0]) {
        Ok(stat) => {
            if !ptr.is_null() {
                unsafe { ptr.write_volatile(stat) };
                0
            } else {
                (-vfs_error_to_errno(VfsError::FileNameIllegal)) as u64
            }
        }
        Err(e) => (-vfs_error_to_errno(e)) as u64,
    };
}

/// stat(2)/lstat(6) share this body: MOS has no symlinks yet, so lstat == stat.
pub fn sys_stat_path(path_ptr: u64) -> VfsResult<Stat> {
    let path = unsafe { c_str_to_str(path_ptr as _) }.map_err(|_| VfsError::FileNameIllegal)?;
    let current_tcb = current_tcb();
    let work_dir = current_tcb.process.work_dir().lock().clone();
    let file = mos_fs_runtime::open_with_dentry(
        mos_vfs::vfs_root_dentry(),
        work_dir,
        path,
        OpenFlags::READ,
    )?;
    stat_from_file(&file)
}
