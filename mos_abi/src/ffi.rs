#[cfg(feature = "alloc")]
use alloc::ffi::CString;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::ffi::{CStr, c_char};
use core::str::Utf8Error;

const fn find_null<T>(elems: &[*const T]) -> Option<usize> {
    let mut i = 0;
    while i < elems.len() {
        if elems[i].is_null() {
            return Some(i);
        }
        i += 1;
    }
    None
}

const fn find_null_from_ptr<T>(elems: *const *const T) -> Option<usize> {
    if elems.is_null() {
        return None;
    }
    let mut i = 0;
    unsafe {
        while !(*elems.add(i)).is_null() {
            i += 1;
        }
    }
    Some(i)
}

/// # Safety
/// caller must ensure the pointer point field is legal to access
/// return safe array with tail nullptr
pub const unsafe fn null_end_pointer_array<'a, T>(ptr: *const *const T) -> &'a [*const T] {
    let index = find_null_from_ptr(ptr);
    match index {
        None => &[core::ptr::null()],
        Some(len) => unsafe { core::slice::from_raw_parts(ptr, len + 1) },
    }
}

/// # Safety
/// the ptr must legal for CStr::from_ptr(ptr)
pub const unsafe fn c_str_to_str(ptr: *const c_char) -> Result<&'static str, Utf8Error> {
    unsafe { CStr::from_ptr(ptr).to_str() }
}

#[derive(Debug)]
pub struct NullEndPointerArray<'a, T> {
    data: &'a [*const T],
}
impl<'a, T> NullEndPointerArray<'a, T> {
    pub fn from_slice(data: &'a [*const T]) -> Option<Self> {
        if let Some(ptr) = data.last()
            && ptr.is_null()
        {
            Some(Self { data })
        } else {
            None
        }
    }
    /// if data is null, it will act like &[]
    pub unsafe fn from_ptr(data: *const *const T) -> Self {
        unsafe {
            Self {
                data: null_end_pointer_array(data),
            }
        }
    }

    pub fn as_array(&self) -> &[*const T] {
        &self.data[..self.data.len() - 1]
    }
    pub fn as_array_with_null(&self) -> &[*const T] {
        self.data
    }
}

#[cfg(feature = "alloc")]
impl<'a> NullEndPointerArray<'a, c_char> {
    pub fn to_vec_cstring(&self) -> Vec<CString> {
        unsafe {
            self.as_array()
                .iter()
                .map(|&e| CString::new(CStr::from_ptr(e).to_bytes().to_vec()))
                .flatten()
                .collect()
        }
    }
}
