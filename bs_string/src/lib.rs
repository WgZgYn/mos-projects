#![no_std]

use core::borrow::Borrow;
use core::ops::{Index, IndexMut};
use core::slice;
use core::str::Utf8Error;

#[derive(Clone)]
pub struct BString<const N: usize> {
    data: [u8; N],
    len: usize,
}

#[repr(transparent)]
pub struct BStr {
    data: [u8],
}

impl<const N: usize> AsRef<[u8]> for BString<N> {
    fn as_ref(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

impl<const N: usize> AsRef<BStr> for BString<N> {
    fn as_ref(&self) -> &BStr {
        self.as_bstr()
    }
}

impl<const N: usize> Extend<u8> for BString<N> {
    fn extend<T: IntoIterator<Item = u8>>(&mut self, iter: T) {
        iter.into_iter().for_each(|b| self.push_byte(b))
    }
}

impl<'a, const N: usize> Extend<&'a u8> for BString<N> {
    fn extend<T: IntoIterator<Item = &'a u8>>(&mut self, iter: T) {
        iter.into_iter().for_each(|b| self.push_byte(*b))
    }
}

impl<const N: usize> BString<N> {
    pub fn push_byte(&mut self, b: u8) {
        self.data[self.len] = b;
        self.len += 1;
    }
    pub fn try_push_byte(&mut self, b: u8) -> Result<(), &'static str> {
        if self.len >= N {
            return Err("too many bytes");
        }
        self.push_byte(b);
        Ok(())
    }
    pub fn push_str(&mut self, s: &str) {
        self.extend(s.bytes())
    }
    pub fn try_push_str(&mut self, s: &str) -> Result<(), &'static str> {
        if self.len + s.len() >= N {
            return Err("too many bytes");
        }
        self.push_str(s);
        Ok(())
    }
    pub fn reverse(&mut self) {
        self.data[..self.len].reverse();
    }
}

impl<const N: usize> BString<N> {
    pub fn as_bstr(&'_ self) -> &BStr {
        BStr::from_bytes(self.as_ref())
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.data[..self.len]
    }
}

impl<const N: usize> BString<N> {
    pub const fn new() -> Self {
        Self {
            data: [0; N],
            len: 0,
        }
    }
}

impl BStr {
    pub fn try_as_str(&self) -> Result<&str, Utf8Error> {
        core::str::from_utf8(&self.data)
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.data
    }
}

impl BStr {
    pub fn from_bytes(slice: &[u8]) -> &Self {
        // # Safety BStr is transparent as [u8]
        unsafe { &*(slice as *const [u8] as *const Self) }
    }
}

impl<const N: usize> core::fmt::Write for BString<N> {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

impl<I, const N: usize> Index<I> for BString<N>
where
    I: slice::SliceIndex<[u8]>,
{
    type Output = I::Output;

    fn index(&self, index: I) -> &Self::Output {
        self.data.index(index)
    }
}

impl<I, const N: usize> IndexMut<I> for BString<N>
where
    I: slice::SliceIndex<[u8]>,
{
    fn index_mut(&mut self, index: I) -> &mut Self::Output {
        self.data.index_mut(index)
    }
}

impl<const N: usize> Borrow<BStr> for BString<N> {
    fn borrow(&self) -> &BStr {
        self.as_bstr()
    }
}
