#![no_std]

pub trait Read {
    type Error;
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

pub trait Write {
    type Error;
    fn write(&mut self, buf: &[u8]) -> Result<usize, Self::Error>;
}

pub trait File {
    type Error;
    fn read_at(&mut self, buf: &mut [u8], offset: u64) -> Result<usize, Self::Error>;
    fn write_at(&mut self, buf: &[u8], offset: u64) -> Result<usize, Self::Error>;
    fn len(&self) -> u64;
}

pub enum OpenFlag {
    ReadOnly,
    WriteOnly,
    ReadWrite,
    Binary,
    Append,
}

pub trait Path {}

pub trait FileSystem {
    type Error;
    type FileType;
    type File: Read + Write;

    fn open(&self, path: &str, open_flag: OpenFlag) -> Result<Self::File, Self::Error>;
    fn create(&self, path: &str, file_type: Self::FileType) -> Result<Self::File, Self::Error>;
    fn remove(&self, path: &str) -> Result<(), Self::Error>;
    fn close(&self, file: Self::File) -> Result<(), Self::Error>;
    fn exists(&self, path: &str) -> bool {
        self.open(path, OpenFlag::ReadOnly).is_ok()
    }
}
