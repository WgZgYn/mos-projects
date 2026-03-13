// use core::ops::Deref;
// use core::str::Utf8Error;

// #[repr(C)]
// #[derive(Debug)]
// pub struct Slice<T> {
//     ptr: *const T,
//     len: usize,
// }
//
// #[repr(C)]
// #[derive(Debug)]
// pub struct BStr(Slice<u8>);
//
// impl<T> Slice<T> {
//     pub fn from_slice(slice: &[T]) -> Self {
//         Self::from_ptr(slice.as_ptr(), slice.len())
//     }
//     pub fn from_ptr(ptr: *const T, len: usize) -> Self {
//         Self { ptr, len }
//     }
//     pub unsafe fn as_slice(&self) -> &[T] {
//         unsafe { core::slice::from_raw_parts(self.ptr, self.len) }
//     }
//     pub fn as_ptr(&self) -> *const T {
//         self.ptr
//     }
//     pub fn len(&self) -> usize {
//         self.len
//     }
//     pub unsafe fn iter(&self) -> impl Iterator<Item = &T> + DoubleEndedIterator {
//         unsafe { self.as_slice().iter() }
//     }
// }
//
// impl Deref for BStr {
//     type Target = Slice<u8>;
//
//     fn deref(&self) -> &Self::Target {
//         &self.0
//     }
// }
//
// impl BStr {
//     pub fn from_str(str: &str) -> Self {
//         Self::from_ptr(str.as_ptr(), str.len())
//     }
//     pub fn from_ptr(ptr: *const u8, len: usize) -> Self {
//         Self(Slice::from_ptr(ptr, len))
//     }
//     pub unsafe fn as_str(&self) -> Result<&str, Utf8Error> {
//         unsafe { core::str::from_utf8(self.as_slice()) }
//     }
//     pub unsafe fn as_str_unchecked(&self) -> &str {
//         unsafe { core::str::from_utf8_unchecked(self.as_slice()) }
//     }
// }
//
// pub type OsStr = BStr;
// pub type OsSlice<T> = Slice<T>;

// #[repr(transparent)]
// struct PtrsWithNul<T> {
//     inner: [*const T],
// }
//
// pub enum FromElemsWithNulError {}
//
// pub struct FromElemsUntilNulError;
//
// impl<T> PtrsWithNul<T> {
//     pub const unsafe fn from_ptr<'a>(ptr: *const [*const T]) -> &'a Self {
//         unsafe {
//             let mut len = 0;
//             while !(*ptr)[len].is_null() {
//                 len += 1;
//             }
//             Self::from_elems_with_nul_unchecked(core::slice::from_raw_parts((*ptr).as_ptr(), len))
//         }
//     }
//
//     pub const unsafe fn from_elems_with_nul_unchecked(elems: &[*const T]) -> &Self {
//         unsafe { &*(elems as *const [*const T] as *const Self) }
//     }
//
//     pub const fn from_elems_until_nul(elems: &[*const T]) -> Result<&Self, FromElemsUntilNulError> {
//         let nul_pos = find_null(elems);
//         match nul_pos {
//             Some(nul_pos) => {
//                 let subslice = unsafe { core::slice::from_raw_parts(elems.as_ptr(), nul_pos + 1) };
//                 Ok(unsafe { Self::from_elems_with_nul_unchecked(subslice) })
//             }
//             None => Err(FromElemsUntilNulError),
//         }
//     }
// }
//
// pub type StrWithNul = core::ffi::CStr;

#[cfg(feature = "alloc")]
use alloc::ffi::CString;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

use core::ffi::{CStr, c_char};

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
