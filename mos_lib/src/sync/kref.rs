use core::marker::Unsize;
use core::mem::ManuallyDrop;
use core::ops::{CoerceUnsized, Deref};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicUsize, Ordering, fence};
use core::task::{RawWaker, RawWakerVTable, Waker};

/// trait for intrusive refcount structure
pub trait RefCounted {
    /// get the intrusive refcount atomic
    fn ref_count(&self) -> &AtomicUsize;

    /// # Safety
    /// don't call directly, it should be called by KRef
    unsafe fn destroy(&self);
}

/// atomic ref count on intrusive structure
/// it only has strong ref which is different from Arc
/// the mem of T is manually controlled
/// if refcount down to 0, it will call T::destroy
/// it's not 100% safe, and it's created by ptr/ref to the already exist data,
/// so it need there are no other ref to the data
pub struct KRef<T: RefCounted + ?Sized> {
    ptr: NonNull<T>,
}

impl<T: RefCounted + ?Sized> KRef<T> {
    /// # Safety
    /// construct the KRef<T> from ptr
    /// need no other ref
    pub const unsafe fn from_ptr(ptr: NonNull<T>) -> Self {
        Self { ptr }
    }

    #[allow(clippy::wrong_self_convention)]
    pub fn into_ptr(this: Self) -> NonNull<T> {
        let m = ManuallyDrop::new(this);
        m.ptr
    }

    /// # Safety
    /// inc the count
    pub unsafe fn increment_strong_count(this: NonNull<T>) {
        unsafe {
            let p = ManuallyDrop::new(Self::from_ptr(this));
            let _: ManuallyDrop<Self> = ManuallyDrop::new(p.deref().clone());
        }
    }

    /// # Safety
    /// dec the count
    pub unsafe fn decrement_strong_count(this: NonNull<T>) {
        unsafe {
            let _ = Self::from_ptr(this);
        }
    }
}

impl<T: RefCounted + ?Sized> Deref for KRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }
    }
}

impl<T: RefCounted + ?Sized> Clone for KRef<T> {
    fn clone(&self) -> Self {
        self.ref_count().fetch_add(1, Ordering::Relaxed);
        Self { ptr: self.ptr }
    }
}

impl<T: RefCounted + ?Sized> Drop for KRef<T> {
    fn drop(&mut self) {
        if self.ref_count().fetch_sub(1, Ordering::Release) != 1 {
            return;
        }
        fence(Ordering::Acquire);
        unsafe { T::destroy(self) }
    }
}

unsafe impl<T: RefCounted> Sync for KRef<T> {}
unsafe impl<T: RefCounted> Send for KRef<T> {}
impl<T, U> CoerceUnsized<KRef<U>> for KRef<T>
where
    T: RefCounted + Unsize<U>,
    U: RefCounted + ?Sized,
{
}

pub trait KWake: Sized + RefCounted {
    fn wake(this: KRef<Self>);
    fn wake_by_ref(this: &KRef<Self>) {
        Self::wake(this.clone());
    }
}

impl<W: KWake + Send + Sync + 'static> From<KRef<W>> for Waker {
    fn from(value: KRef<W>) -> Self {
        fn raw_waker<W: KWake + Send + Sync + 'static>(waker: KRef<W>) -> RawWaker {
            // Increment the reference count of the arc to clone it.
            //
            // The #[inline(always)] is to ensure that raw_waker and clone_waker are
            // always generated in the same code generation unit as one another, and
            // therefore that the structurally identical const-promoted RawWakerVTable
            // within both functions is deduplicated at LLVM IR code generation time.
            // This allows optimizing Waker::will_wake to a single pointer comparison of
            // the vtable pointers, rather than comparing all four function pointers
            // within the vtables.
            #[inline(always)]
            unsafe fn clone_waker<W: KWake + Send + Sync + 'static>(waker: *const ()) -> RawWaker {
                unsafe {
                    KRef::increment_strong_count(NonNull::new_unchecked(
                        (waker as *const W).cast_mut(),
                    ))
                };
                RawWaker::new(
                    waker,
                    &RawWakerVTable::new(
                        clone_waker::<W>,
                        wake::<W>,
                        wake_by_ref::<W>,
                        drop_waker::<W>,
                    ),
                )
            }

            // Wake by value, moving the Arc into the Wake::wake function
            unsafe fn wake<W: KWake + Send + Sync + 'static>(waker: *const ()) {
                let waker = unsafe {
                    KRef::from_ptr(NonNull::new_unchecked((waker as *const W).cast_mut()))
                };
                <W as KWake>::wake(waker);
            }

            // Wake by reference, wrap the waker in ManuallyDrop to avoid dropping it
            unsafe fn wake_by_ref<W: KWake + Send + Sync + 'static>(waker: *const ()) {
                let waker = unsafe {
                    ManuallyDrop::new(KRef::from_ptr(NonNull::new_unchecked(
                        (waker as *const W).cast_mut(),
                    )))
                };
                <W as KWake>::wake_by_ref(&waker);
            }

            // Decrement the reference count of the Arc on drop
            unsafe fn drop_waker<W: KWake + Send + Sync + 'static>(waker: *const ()) {
                unsafe {
                    KRef::decrement_strong_count(NonNull::new_unchecked(
                        (waker as *const W).cast_mut(),
                    ))
                };
            }

            RawWaker::new(
                KRef::into_ptr(waker).as_ptr().cast(),
                &RawWakerVTable::new(
                    clone_waker::<W>,
                    wake::<W>,
                    wake_by_ref::<W>,
                    drop_waker::<W>,
                ),
            )
        }

        unsafe { Waker::from_raw(raw_waker(value)) }
    }
}
