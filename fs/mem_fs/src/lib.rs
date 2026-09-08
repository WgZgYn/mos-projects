#![no_std]

extern crate alloc;

use alloc::boxed::Box;
use alloc::string::{String, ToString};
use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;
use vfs_common::{FileType, IoContext, VfsError, VfsResult};
use vfs_traits::*;

lazy_static! {
    static ref FS: SuperBlock = SuperBlock {
        magic: "memfs",
        root: Arc::new_cyclic(|weak| MemInode {
            inner: Mutex::new(InodeInner {
                name: String::from("/"),
                parent: None,
                content: FileContent::new(FileType::Directory),
            },),
            weak: weak.clone(),
        }),
    };
}

pub fn get_memfs_root() -> Arc<dyn Inode> {
    FS.root.clone()
}

struct DataFrames {
    /// make it easy to enlarge, and make it match to 4KiB PageSize
    /// maybe I will use page allocator to ensure the alignment and prevent outer fragment
    data: Vec<Box<[u8; 4096]>>,
    len: usize,
}

impl DataFrames {
    pub const fn new() -> Self {
        Self {
            data: Vec::new(),
            len: 0,
        }
    }

    pub const fn len(&self) -> usize {
        self.len
    }

    /// get the page index and offset
    pub fn page_index_offset(&self) -> (usize, usize) {
        (self.len >> 12, self.len & 0xfff)
    }
}

enum FileContent {
    Dir(Vec<DirEntry>),
    File(DataFrames),
    Fifo,
}

impl FileContent {
    const fn new(file_type: FileType) -> Self {
        match file_type {
            FileType::Regular => FileContent::File(DataFrames::new()),
            FileType::Directory => FileContent::Dir(Vec::new()),
            FileType::Fifo => FileContent::Fifo,
            _ => panic!("not supported"),
        }
    }
    fn size(&self) -> usize {
        match &self {
            FileContent::Dir(d) => d.len() as _,
            FileContent::File(f) => f.len() as _,
            _ => 0,
        }
    }
    fn file_type(&self) -> FileType {
        match &self {
            FileContent::Dir(_) => FileType::Directory,
            FileContent::File(_) => FileType::Regular,
            FileContent::Fifo => FileType::Fifo,
        }
    }
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>> {
        match self {
            FileContent::Dir(d) => d
                .iter()
                .find(|f| f.name() == name)
                .map(|f| f.node())
                .ok_or(VfsError::NotFound),
            _ => Err(VfsError::NotDir),
        }
    }
    fn add_entry(&mut self, entry: DirEntry) -> VfsResult<()> {
        if let FileContent::Dir(d) = self {
            d.push(entry);
            Ok(())
        } else {
            Err(VfsError::NotDir)
        }
    }
    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        if let FileContent::Dir(d) = self {
            Ok(d.get(offset).cloned())
        } else {
            Err(VfsError::NotDir)
        }
    }
}

pub struct SuperBlock {
    magic: &'static str,
    root: Arc<MemInode>,
}

struct InodeInner {
    name: String,
    parent: Option<Arc<MemInode>>,
    content: FileContent,
}

impl InodeInner {
    fn new(name: String, parent: Option<Arc<MemInode>>, content: FileContent) -> Self {
        Self {
            name,
            parent,
            content,
        }
    }
    fn name(&self) -> &str {
        &self.name
    }
}

struct MemInode {
    inner: Mutex<InodeInner>,
    weak: Weak<MemInode>,
}

impl Inode for MemInode {
    fn as_any(&self) -> &dyn core::any::Any {
        self
    }
    fn meta_data(&self) -> &[u8] {
        &[]
    }

    fn file_size(&self) -> u64 {
        self.inner.lock().content.size() as _
    }

    fn file_type(&self) -> FileType {
        self.inner.lock().content.file_type()
    }
    fn parent(&self) -> VfsResult<Option<Arc<dyn Inode>>> {
        Ok(self
            .inner
            .lock()
            .parent
            .clone()
            .map(|i| i as Arc<dyn Inode>))
    }
    fn as_file_ops(&self) -> VfsResult<&dyn FileOps> {
        if self.file_type() == FileType::Regular {
            Ok(self)
        } else {
            Err(VfsError::NotFile)
        }
    }
    fn as_dir_ops(&self) -> VfsResult<&dyn DirOps> {
        if self.file_type() == FileType::Directory {
            Ok(self)
        } else {
            Err(VfsError::NotDir)
        }
    }
}

impl DirOps for MemInode {
    fn lookup(&self, name: &str) -> VfsResult<Arc<dyn Inode>> {
        self.inner.lock().content.lookup(name)
    }

    fn create(&self, name: &str, file_type: FileType) -> VfsResult<Arc<dyn Inode>> {
        let mut guard = self.inner.lock();
        if guard.content.file_type() != FileType::Directory {
            return Err(VfsError::NotDir);
        }
        let parent = self.weak.upgrade().ok_or(VfsError::InternelError)?;
        let node = Arc::new_cyclic(|weak| MemInode {
            inner: Mutex::new(InodeInner::new(
                name.to_string(),
                Some(parent),
                FileContent::new(file_type),
            )),
            weak: weak.clone(),
        });
        let entry = DirEntry::new(name.to_string(), file_type, node.clone());
        guard.content.add_entry(entry)?;
        Ok(node)
    }

    fn readdir(&self, offset: usize) -> VfsResult<Option<DirEntry>> {
        self.inner.lock().content.readdir(offset)
    }
}

impl FileOps for MemInode {
    fn read(&self, _ctx: &IoContext, mut offset: usize, mut buf: &mut [u8]) -> VfsResult<usize> {
        let FileContent::File(f) = &mut self.inner.lock().content else {
            return Err(VfsError::NotFile);
        };

        if offset >= f.len() {
            return Ok(0);
        }

        // this an implement of read_exact
        let mut need_read = buf.len().min(f.len() - offset);
        let mut total_read = 0;

        while need_read > 0 {
            let page_index = offset >> 12;
            let page_offset = offset & 0xfff;

            let n = (4096 - page_offset).min(need_read);
            buf[..n].copy_from_slice(&f.data[page_index][page_offset..page_offset + n]);
            buf = &mut buf[n..];

            need_read -= n;
            total_read += n;
            offset += n;
        }
        Ok(total_read)
    }

    fn write(&self, _ctx: &IoContext, mut offset: usize, mut buf: &[u8]) -> VfsResult<usize> {
        let FileContent::File(f) = &mut self.inner.lock().content else {
            return Err(VfsError::NotFile);
        };

        let pages = (offset + buf.len()).div_ceil(4096);

        f.data
            .resize(pages, unsafe { Box::new_zeroed().assume_init() });

        let mut need_write = buf.len();
        let mut total_write = 0;
        while need_write > 0 {
            let page_index = offset >> 12;
            let page_offset = offset & 0xfff;

            let n = (4096 - page_offset).min(need_write);
            f.data[page_index][page_offset..page_offset + n].copy_from_slice(&buf[..n]);
            buf = &buf[n..];

            need_write -= n;
            offset += n;
            total_write += n;
        }
        f.len += total_write;
        Ok(total_write)
    }
}

mod tests {
    use crate::get_memfs_root;
    use vfs_common::{FileType, IoContext};

    #[test]
    fn test_all() {
        let root = get_memfs_root();
        let text = root
            .as_dir_ops()
            .unwrap()
            .create("a.txt", FileType::Regular)
            .unwrap();
        let n = text
            .as_file_ops()
            .unwrap()
            .write(&IoContext::default(), 0, b"hello");
        assert_eq!(n.unwrap(), 5);
        let mut buffer = [0; 10];
        let n = text
            .as_file_ops()
            .unwrap()
            .read(&IoContext::default(), 0, &mut buffer);
        assert_eq!(n.unwrap(), 5);
        assert_eq!(&buffer[..5], b"hello");
    }
}
