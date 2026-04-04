use super::phys::PhysicalFrameManager;
use super::phys_to_virt;
use alloc::vec::Vec;
use core::marker::PhantomData;
use core::mem::MaybeUninit;
use core::ops::{Index, IndexMut};
use x86_64::structures::paging::{FrameAllocator, Page, PageSize, Size4KiB};

struct PageOf<T> {
    page: Page<Size4KiB>,
    _phantom: PhantomData<[MaybeUninit<T>]>,
}

impl<T> PageOf<T> {
    const _LARGE_OBJ: bool = size_of::<T>() > 4096;
    const _ZST: bool = size_of::<T>() == 0;
    const _NOT_ALIGNED: bool = 4096 % align_of::<T>() != 0;
    const _CHECK: () = assert!(!Self::_LARGE_OBJ && !Self::_ZST && !Self::_NOT_ALIGNED);

    fn alloc() -> Self {
        let _ = Self::_CHECK;

        let frame = PhysicalFrameManager
            .global_allocate_frame()
            .expect("should not fail");
        let paddr = frame.start_address();
        let vaddr = phys_to_virt(paddr);
        Self {
            page: Page::containing_address(vaddr),
            _phantom: PhantomData,
        }
    }

    fn alloc_from(provider: &mut impl FrameAllocator<Size4KiB>) -> Self {
        let _ = Self::_CHECK;

        let frame = provider.allocate_frame().expect("should not fail");
        let paddr = frame.start_address();
        let vaddr = phys_to_virt(paddr);
        Self {
            page: Page::containing_address(vaddr),
            _phantom: PhantomData,
        }
    }

    fn max_elements() -> usize {
        Size4KiB::SIZE as usize / size_of::<MaybeUninit<T>>()
    }
    fn get(&self, index: usize) -> Option<&MaybeUninit<T>> {
        if index >= Self::max_elements() {
            return None;
        }
        unsafe { Some(self.get_unchecked(index)) }
    }
    fn get_mut(&mut self, index: usize) -> Option<&mut MaybeUninit<T>> {
        if index >= Self::max_elements() {
            return None;
        }
        unsafe { Some(self.get_mut_unchecked(index)) }
    }
    unsafe fn get_unchecked(&self, index: usize) -> &MaybeUninit<T> {
        let base = self.page.start_address().as_ptr::<MaybeUninit<T>>();
        unsafe { &*base.add(index) }
    }
    unsafe fn get_mut_unchecked(&mut self, index: usize) -> &mut MaybeUninit<T> {
        let base = self.page.start_address().as_mut_ptr::<MaybeUninit<T>>();
        unsafe { &mut *base.add(index) }
    }
}

enum Entry<T> {
    // the usize is used as embedded link to next Vacant
    Vacant(usize),
    Occupied(T),
}

impl<T> Entry<T> {
    fn in_occupied(&self) -> bool {
        matches!(self, Entry::Occupied(_))
    }
}

struct SlabInner<T> {
    pages: Vec<PageOf<Entry<T>>>,

    /// the num of init entry
    init: usize,
}

impl<T> SlabInner<T> {
    pub const fn new() -> Self {
        SlabInner {
            pages: Vec::new(),
            init: 0,
        }
    }
    pub fn get(&self, index: usize) -> Option<&Entry<T>> {
        if index >= self.init {
            return None;
        }
        let offset = index % PageOf::<Entry<T>>::max_elements();
        let page_index = index / PageOf::<Entry<T>>::max_elements();
        self.pages
            .get(page_index)?
            .get(offset)
            .map(|e| unsafe { e.assume_init_ref() })
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Entry<T>> {
        if index >= self.init {
            return None;
        }
        let offset = index % PageOf::<Entry<T>>::max_elements();
        let page_index = index / PageOf::<Entry<T>>::max_elements();
        self.pages
            .get_mut(page_index)?
            .get_mut(offset)
            .map(|e| unsafe { e.assume_init_mut() })
    }

    pub fn push(&mut self, entry: Entry<T>) -> &mut Entry<T> {
        let offset = self.init % PageOf::<Entry<T>>::max_elements();
        let page_index = self.init / PageOf::<Entry<T>>::max_elements();
        self.init += 1;
        match self.pages.get(page_index) {
            None => {
                let page = self.pages.push_mut(PageOf::alloc());
                unsafe { page.get_mut_unchecked(0).write(entry) }
            }
            Some(_) => unsafe {
                let page = self.pages.get_unchecked_mut(page_index);
                page.get_mut_unchecked(offset).write(entry)
            },
        }
    }

    pub fn push_mut(&mut self, entry: T) -> &mut T {
        let entry = self.push(Entry::Occupied(entry));
        match entry {
            Entry::Occupied(val) => val,
            Entry::Vacant(_) => unreachable!(),
        }
    }
}

/// Invariant:
/// - 0 <= last_vacant <= init
/// - if last_vacant == init => freelist empty
/// - if last_vacant < init => entries[last_vacant] must be Vacant
/// TODO: add gen to entry
pub struct Slab<T> {
    entries: SlabInner<T>,
    last_vacant: usize,
    len: usize,
}

impl<T> Default for Slab<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Slab<T> {
    pub const fn new() -> Self {
        Self {
            entries: SlabInner::new(),
            last_vacant: 0,
            len: 0,
        }
    }
    pub fn insert(&mut self, val: T) -> usize {
        let key = self.last_vacant;
        self.insert_at(key, val);
        key
    }

    pub fn insert_mut(&mut self, val: T) -> (usize, &mut T) {
        let key = self.last_vacant;
        (key, self.insert_at(key, val))
    }

    fn insert_at(&mut self, key: usize, val: T) -> &mut T {
        self.len += 1;

        match self.entries.get(key) {
            None => {
                self.last_vacant += 1;
                self.entries.push_mut(val)
            }
            Some(_) => {
                if let Some(entry @ &mut Entry::Vacant(vacant)) = self.entries.get_mut(key) {
                    self.last_vacant = vacant;
                    *entry = Entry::Occupied(val);
                    match entry {
                        Entry::Occupied(v) => v,
                        _ => unreachable!(),
                    }
                } else {
                    panic!("entry already occupied");
                }
            }
        }
    }

    pub fn get(&self, key: usize) -> Option<&T> {
        match self.entries.get(key) {
            Some(Entry::Occupied(val)) => Some(val),
            _ => None,
        }
    }

    pub fn get_mut(&mut self, key: usize) -> Option<&mut T> {
        match self.entries.get_mut(key) {
            Some(Entry::Occupied(val)) => Some(val),
            _ => None,
        }
    }

    pub fn remove(&mut self, key: usize) -> T {
        match self.entries.get_mut(key) {
            Some(v) if v.in_occupied() => {
                let ret = core::mem::replace(v, Entry::Vacant(self.last_vacant));
                let ret = match ret {
                    Entry::Occupied(v) => v,
                    Entry::Vacant(_) => unreachable!(),
                };
                self.len -= 1;
                self.last_vacant = key;
                ret
            }
            _ => panic!("invalid key"),
        }
    }

    pub fn remove_inplace(&mut self, key: usize) {
        match self.entries.get_mut(key) {
            Some(v) if v.in_occupied() => {
                // T will drop here
                *v = Entry::Vacant(self.last_vacant);
                self.len -= 1;
                self.last_vacant = key;
            }
            _ => panic!("invalid key"),
        }
    }

    pub fn insert_with_key<F>(&mut self, f: F) -> &mut T
    where
        F: FnOnce(usize) -> T,
    {
        let v = f(self.last_vacant);
        self.insert_at(self.last_vacant, v)
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T> Index<usize> for Slab<T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        self.get(index).expect("index out of bounds")
    }
}

impl<T> IndexMut<usize> for Slab<T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.get_mut(index).expect("index out of bounds")
    }
}

pub fn test_slab() {
    let mut slab = Slab::new();

    let hello = slab.insert("hello");
    let world = slab.insert("world");

    assert_eq!(slab[hello], "hello");
    assert_eq!(slab[world], "world");

    slab[world] = "earth";
    assert_eq!(slab[world], "earth");

    let v = slab.insert_with_key(|_key| "!");
    assert_eq!(*v, "!");
}
