use crate::constant::RAMFS_FRAME_SIZE_BYTE;
use crate::file::File;
use crate::fs::Error;
use crate::fs_manager::RamFsManager;
use kernel_utils::{error, trace};
use vfs::{Read, Write};

impl Read for File {
    type Error = Error;

    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Error> {
        trace!("file read");
        let gfs = RamFsManager::global().map_err(|_| Error::FileSystemError)?;
        let fs = gfs.lock();
        let inode = &fs[self.fd];

        // 超出文件大小，直接 EOF
        if self.cursor as u64 >= inode.size {
            return Ok(0);
        }

        trace!("file size: {}", inode.size);
        let total_available = (inode.size as usize - self.cursor).min(buf.len());
        let mut read_total = 0;

        while read_total < total_available {
            let frame_idx = self.cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = self.cursor % RAMFS_FRAME_SIZE_BYTE;

            let frame_idx = fs
                .get_inode_nth_frame(self.fd, frame_idx)
                .to_option()
                .ok_or(Error::FileSystemError)?;

            let frame = &fs[frame_idx];

            // 当前 frame 剩余可读空间
            let frame_remain = RAMFS_FRAME_SIZE_BYTE - offset;

            // 总剩余可读字节
            let remaining = total_available - read_total;

            // 本次读取长度
            let n = frame_remain.min(remaining);

            buf[read_total..read_total + n].copy_from_slice(&frame[offset..offset + n]);

            self.cursor += n;
            read_total += n;
        }

        Ok(read_total)
    }
}

impl Write for File {
    type Error = Error;

    fn write(&mut self, mut buf: &[u8]) -> Result<usize, Error> {
        let gfs = RamFsManager::global().map_err(|_| Error::FileSystemError)?;
        let mut fs = gfs.lock();

        let mut total_written = 0;
        while !buf.is_empty() {
            let frame_nth = self.cursor / RAMFS_FRAME_SIZE_BYTE;
            let offset = self.cursor % RAMFS_FRAME_SIZE_BYTE;

            trace!(
                "cursor: {}, frame_nth: {frame_nth}, offset: {offset}",
                self.cursor
            );
            let frame_idx = fs.get_or_init_inode_nth_frame(self.fd, frame_nth)?;
            trace!("write to frame idx: {:?}", frame_idx);
            let frame = &mut fs[frame_idx];

            let writable = (RAMFS_FRAME_SIZE_BYTE - offset).min(buf.len());
            frame[offset..offset + writable].copy_from_slice(&buf[..writable]);

            let inode = &mut fs[self.fd];

            self.cursor += writable;
            total_written += writable;
            if self.cursor > inode.size as usize {
                inode.size = self.cursor as u64;
            }
            buf = &buf[writable..];
        }

        Ok(total_written)
    }
}

impl core::fmt::Write for File {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.write(s.as_bytes()).map(|_| ()).map_err(|e| {
            error!("write error: {:?}", e);
            core::fmt::Error
        })
    }
}
