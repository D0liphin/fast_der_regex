use std::ptr::NonNull;


/// This class is only meant so that I can make sure that I am not using any mutating methods on
/// the internal pointer. It still needs to be constructed from some kind of mutable pointer.
#[derive(Debug)]
pub struct Const<T>(NonNull<T>);

impl<T> Const<T> {
    pub fn new(inner: NonNull<T>) -> Self {
        Self(inner)
    }

    pub fn dangling() -> Self {
        Self(NonNull::dangling())
    }

    pub unsafe fn read(&self) -> T {
        self.0.as_ptr().read()
    }

    pub unsafe fn as_ref(&self) -> &T {
        self.0.as_ref()
    }

    /// Performs pointer equality on two `Const<T>`s. Always safe.
    pub fn ptr_eq(self, rhs: Self) -> bool {
        self.0 == rhs.0
    }

    /// SAFETY: `self` and `rhs` will be dereferenced and read. Aliasing safety is probably not a
    /// concern if you are exclusively using `Const`, but the pointers could be dangling.
    pub unsafe fn eq(self, rhs: Self, inner_eq: impl Fn(&T, &T) -> bool) -> bool {
        self.ptr_eq(rhs) || unsafe { inner_eq(self.as_ref(), rhs.as_ref()) }
    }
}

impl<T> Clone for Const<T> {
    fn clone(&self) -> Self {
        Const(self.0)
    }
}

impl<T> Copy for Const<T> {}

impl<T> From<NonNull<T>> for Const<T> {
    fn from(value: NonNull<T>) -> Self {
        Self::new(value)
    }
}

impl<T> From<&T> for Const<T> {
    fn from(value: &T) -> Self {
        Self::new(value.into())
    }
}