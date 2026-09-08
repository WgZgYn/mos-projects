//! mkdir(2)/unlink(2)/rmdir(2)/rename(2)/link(2) — the directory-management
//! syscalls busybox needs for `mkdir`, `rm`, `mv`, `ln`, `cp -r`.

use alloc::borrow::ToOwned;
use alloc::string::String;
use alloc::sync::Arc;
use mos_abi::ffi::c_str_to_str;
use mos_abi::fs::FileDesc;
use mos_arch::structs::SyscallContext;
use mos_task::current_tcb;
use mos_vfs::dentry::Dentry;
use mos_vfs::{vfs_lookup, vfs_root_dentry, VfsLookUpResult};
use vfs_common::{FileType, VfsError, VfsResult, vfs_result_to_isize};

/// Resolve a path against the current work dir; the last component must not
/// exist yet, return its parent dir and name.
fn resolve_parent(path: &str) -> VfsResult<(Arc<Dentry>, String)> {
    let work_dir = current_tcb().process.work_dir().lock().clone();
    match vfs_lookup(vfs_root_dentry(), work_dir, &path) {
        VfsLookUpResult::Last(dir, name) => Ok((dir, name.to_owned())),
        VfsLookUpResult::Ok(_) => Err(VfsError::AlreadyExists),
        VfsLookUpResult::Err(e) => Err(e),
    }
}

/// Resolve a full path; the final component must exist.
fn resolve_existing(path: &str) -> VfsResult<Arc<Dentry>> {
    let work_dir = current_tcb().process.work_dir().lock().clone();
    match vfs_lookup(vfs_root_dentry(), work_dir, &path) {
        VfsLookUpResult::Ok(dentry) => Ok(dentry),
        VfsLookUpResult::Last(_, _) => Err(VfsError::NotFound),
        VfsLookUpResult::Err(e) => Err(e),
    }
}

pub fn sys_mkdir(path: &str, _mode: u32) -> VfsResult<()> {
    let (dir, name) = resolve_parent(path)?;
    dir.create(&name, FileType::Directory)?;
    Ok(())
}

pub fn sys_unlink(path: &str) -> VfsResult<()> {
    let dentry = resolve_existing(path)?;
    if dentry.file_type()? == FileType::Directory {
        return Err(VfsError::NotFile); // EISDIR
    }
    let parent = dentry.parent().ok_or(VfsError::NotFound)?;
    parent.remove(&dentry.name())
}

pub fn sys_rmdir(path: &str) -> VfsResult<()> {
    let dentry = resolve_existing(path)?;
    if dentry.file_type()? != FileType::Directory {
        return Err(VfsError::NotDir); // ENOTDIR
    }
    // must be empty
    let inode = dentry.inode().ok_or(VfsError::NotFound)?;
    let inner_inode = inode.inode();
    let dir_ops = inner_inode.as_dir_ops()?;
    if dir_ops.readdir(0)?.is_some() {
        return Err(VfsError::NotEmpty); // ENOTEMPTY
    }
    let parent = dentry.parent().ok_or(VfsError::NotFound)?;
    parent.remove(&dentry.name())
}

pub fn sys_rename(old: &str, new: &str) -> VfsResult<()> {
    let old_dentry = resolve_existing(old)?;
    let parent = old_dentry.parent().ok_or(VfsError::NotFound)?;
    let existing_target = resolve_existing(new).ok();
    // both paths must share the same parent dir (no cross-dir move yet)
    let (new_parent, new_name, target_dentry) = match &existing_target {
        Some(d) => (
            d.parent().ok_or(VfsError::NotFound)?,
            d.name(),
            Some(d.clone()),
        ),
        None => {
            let (p, n) = resolve_parent(new)?;
            (p, n, None)
        }
    };
    if !Arc::ptr_eq(&parent, &new_parent) {
        return Err(VfsError::NotSupported); // EXDEV-ish
    }
    // replacing an existing target: remove it first (dirs must be empty)
    if let Some(target) = target_dentry {
        if target.file_type()? == FileType::Directory {
            let inode = target.inode().ok_or(VfsError::NotFound)?;
            let inner_inode = inode.inode();
            if inner_inode.as_dir_ops()?.readdir(0)?.is_some() {
                return Err(VfsError::NotEmpty);
            }
        }
        parent.remove(&target.name())?;
    }
    parent.rename(&old_dentry.name(), &new_name)
}

pub fn sys_link(old: &str, new: &str) -> VfsResult<()> {
    let old_dentry = resolve_existing(old)?;
    if old_dentry.file_type()? == FileType::Directory {
        return Err(VfsError::PermissionDenied); // EPERM on dirs
    }
    let inode = old_dentry.inode().ok_or(VfsError::NotFound)?;
    let (new_parent, new_name) = resolve_parent(new)?;
    new_parent.link(&new_name, &inode)
}

// ---- symlink / readlink / access / umask / truncate / chmod ----

pub fn sys_symlink(target: &str, linkpath: &str) -> VfsResult<()> {
    let (dir, name) = resolve_parent(linkpath)?;
    let dentry = dir.create(&name, FileType::SymLink)?;
    let inode = dentry.inode().ok_or(VfsError::NotFound)?;
    let inner = inode.inode();
    let ops = inner.as_file_ops()?;
    let n = ops.write(&vfs_common::IoContext::default(), 0, target.as_bytes())?;
    if n != target.len() {
        return Err(VfsError::NoSpace);
    }
    Ok(())
}

pub fn sys_readlink(path: &str, buf: &mut [u8]) -> VfsResult<usize> {
    let dentry = resolve_existing(path)?;
    if dentry.file_type()? != FileType::SymLink {
        return Err(VfsError::FileNameIllegal); // EINVAL
    }
    let inode = dentry.inode().ok_or(VfsError::NotFound)?;
    let inner = inode.inode();
    let ops = inner.as_file_ops()?;
    ops.read(&vfs_common::IoContext::default(), 0, buf)
}

/// access(2): F_OK(0) = existence; mode bits are not enforced (no perms yet).
pub fn sys_access(path: &str, _mode: u32) -> VfsResult<()> {
    resolve_existing(path).map(|_| ())
}

pub fn sys_umask(mask: u32) -> u32 {
    let tcb = current_tcb();
    let old = tcb.process.umask.load(core::sync::atomic::Ordering::Acquire);
    tcb.process
        .umask
        .store(mask & 0o777, core::sync::atomic::Ordering::Release);
    old
}

pub fn sys_truncate(path: &str, len: u64) -> VfsResult<()> {
    let dentry = resolve_existing(path)?;
    if dentry.file_type()? != FileType::Regular {
        return Err(VfsError::FileNameIllegal);
    }
    let inode = dentry.inode().ok_or(VfsError::NotFound)?;
    inode.inode().truncate(len)
}

pub fn sys_ftruncate(fd: FileDesc, len: u64) -> VfsResult<()> {
    let file = current_tcb()
        .process
        .files()
        .lock()
        .get(fd)
        .cloned()
        .ok_or(VfsError::BadFileDesc)?;
    file.inode().truncate(len)
}

pub fn sys_chmod(path: &str, _mode: u32) -> VfsResult<()> {
    // no permission bits stored yet, but the path must exist
    resolve_existing(path).map(|_| ())
}

pub fn sys_fchmod(_fd: FileDesc, _mode: u32) -> VfsResult<()> {
    Ok(())
}

// ---- raw wrappers (Linux -errno convention) ----

fn path_arg(ptr: u64) -> VfsResult<String> {
    let path = unsafe { c_str_to_str(ptr as _) };
    match path {
        Ok(p) => {
            let p: String = p.into();
            Ok(p)
        }
        Err(_) => Err(VfsError::FileNameIllegal),
    }
}

pub fn raw_sys_mkdir(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_mkdir(&p, args[1] as _)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_unlink(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_unlink(&p)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_rmdir(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_rmdir(&p)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_rename(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|old| {
        path_arg(args[1]).and_then(|new| sys_rename(&old, &new))
    }) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_link(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|old| {
        path_arg(args[1]).and_then(|new| sys_link(&old, &new))
    }) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_symlink(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0])
        .and_then(|target| path_arg(args[1]).and_then(|link| sys_symlink(&target, &link)))
    {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_readlink(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|path| {
        let buf: &mut [u8] = unsafe { core::slice::from_raw_parts_mut(args[1] as _, args[2] as _) };
        sys_readlink(&path, buf)
    }) {
        Ok(n) => n as u64,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_access(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_access(&p, args[1] as _)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_faccessat(context: &mut SyscallContext, args: [u64; 6]) {
    // dirfd is ignored for absolute paths; flags==0 behaves like access(2)
    context.scratch.rax = match path_arg(args[1]).and_then(|p| sys_access(&p, args[2] as _)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_umask(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = sys_umask(args[0] as u32) as u64;
}

pub fn raw_sys_truncate(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_truncate(&p, args[1])) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_ftruncate(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match sys_ftruncate(FileDesc(args[0] as usize), args[1]) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_chmod(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match path_arg(args[0]).and_then(|p| sys_chmod(&p, args[1] as _)) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}

pub fn raw_sys_fchmod(context: &mut SyscallContext, args: [u64; 6]) {
    context.scratch.rax = match sys_fchmod(FileDesc(args[0] as usize), args[1] as _) {
        Ok(()) => 0,
        Err(e) => vfs_result_to_isize(Err(e)) as _,
    };
}
