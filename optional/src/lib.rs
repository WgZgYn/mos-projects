#![feature(const_trait_impl)]
#![feature(const_cmp)]

pub trait Illegal {
    const ILLEGAL_VALUE: Self;
}
pub trait Constraint {
    fn is_illegal(&self) -> bool;
}

impl<T> Constraint for T
where
    T: Illegal + PartialEq,
{
    fn is_illegal(&self) -> bool {
        self == &T::ILLEGAL_VALUE
    }
}

pub struct Optional<T> {
    inner: T,
}

impl<T> Optional<T> {
    pub const fn new(inner: T) -> Optional<T> {
        Optional { inner }
    }

    pub fn is_some(&self) -> bool
    where
        T: Constraint,
    {
        !self.is_none()
    }

    pub fn is_none(&self) -> bool
    where
        T: Constraint,
    {
        self.inner.is_illegal()
    }

    pub fn unwrap_unchecked(self) -> T {
        self.inner
    }

    pub fn unwrap(self) -> T
    where
        T: Constraint,
    {
        if self.is_none() {
            panic!("called `Optional::unwrap()` on an `None` value");
        } else {
            self.inner
        }
    }

    pub fn to_option(self) -> Option<T>
    where
        T: Constraint,
    {
        if self.is_none() {
            None
        } else {
            Some(self.inner)
        }
    }

    pub fn as_option_ref(&self) -> Option<&T>
    where
        T: Constraint,
    {
        if self.is_none() {
            None
        } else {
            Some(&self.inner)
        }
    }

    pub fn as_option_mut(&mut self) -> Option<&mut T>
    where
        T: Constraint,
    {
        if self.is_none() {
            None
        } else {
            Some(&mut self.inner)
        }
    }
}

impl<T: Constraint> From<Optional<T>> for Option<T> {
    fn from(value: Optional<T>) -> Self {
        value.to_option()
    }
}
