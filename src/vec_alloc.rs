use std::alloc::{Allocator, Global, Layout};
use std::mem::transmute;
use std::ptr::NonNull;
use std::{fmt, ptr};

struct RawBuf<T> {
    data: NonNull<[T]>,
}

impl<T> Drop for RawBuf<T> {
    fn drop(&mut self) {
        unsafe {
            Global.deallocate(
                transmute(self.data.as_non_null_ptr()),
                // SAFETY: this might leak memory, due to rounding errors created in setup. Will
                // have to check this. TODO
                Self::new_layout(self.data.len()).0,
            )
        }
    }
}

impl<T> fmt::Debug for RawBuf<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct RawBuf {
            ptr: *const (),
            capacity: usize,
        }

        let raw_buf = RawBuf {
            ptr: self.data.as_non_null_ptr().as_ptr() as *const (),
            capacity: self.data.len(),
        };

        write!(f, "{:?}", raw_buf)
    }
}

impl<T> RawBuf<T> {
    fn new_layout(capacity: usize) -> (Layout, usize) {
        Layout::new::<T>().repeat(capacity).unwrap()
    }

    /// Create a new
    pub fn new(capacity: usize) -> Self {
        let (layout, offset) = Self::new_layout(capacity);
        let data = Global.allocate(layout).unwrap();
        // SAFETY:
        //     not actually verified, but from my tests, we seem to produce the correct
        //     `Layout` so it should be fine. Maybe it won't be... We'll see. Obviously, just
        //     the creation of this pointer is valid and safe, because it's essentially just a
        //     cast.
        let data = NonNull::slice_from_raw_parts(
            unsafe { transmute(data.as_non_null_ptr()) },
            data.len() / offset,
        );
        Self { data }
    }

    /// ## Safety
    /// - `index` must be less than `self.capacity`.
    pub unsafe fn get_unchecked(&mut self, index: usize) -> NonNull<T> {
        // SAFETY (assuming `index < capacity`):
        //     - `index < self.capacity` so we are in the same allocation
        //     - and thus cannot go out of valid address space
        //     - technically unsafe if the user requests a capacity greater than `isize::MAX` but I
        //       don't care. I think Rust won't allow such allocations anyway, so we're fine.
        let ptr = self.data.as_mut_ptr().add(index);
        // SAFETY: guaranteed to be non-null since we are positively offsetting a non-null pointer.
        NonNull::new_unchecked(ptr)
    }

    /// Returns `None` if the index is out-of-bounds.
    pub fn get(&mut self, index: usize) -> Option<NonNull<T>> {
        if index < self.data.len() {
            // SAFETY: did the exact required bounds check
            Some(unsafe { self.get_unchecked(index) })
        } else {
            None
        }
    }
}

/// Hands out NonNull<T>, packed allocation. Resizable, but previously created pointers will
/// dangle.
pub struct VecAlloc<T> {
    buf: RawBuf<T>,
    len: usize,
}

impl<T> fmt::Debug for VecAlloc<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VecAlloc {{ buf: {:?}, len: {} }}", self.buf, self.len)
    }
}

impl<T> VecAlloc<T> {
    pub fn new(capacity: usize) -> Self {
        Self {
            buf: RawBuf::new(capacity),
            len: 0,
        }
    }

    /// 'Allocate' a new value on this `VecAlloc`. It will be most local to the most-recently
    /// allocated value.
    ///
    /// The resulting `NonNull<T>` is guaranteed to contain `value`. If the allocation fails, it is
    /// because there is no space on the allocator left. The value will be passed-through.
    ///
    /// ## Safety
    /// - Dropping this `VecAlloc` will invalidate all pointers.
    /// - Calling `VecAlloc.resize()` on this allocator will invalidate all allocations,
    ///   dereferencing them is guaranteed UB (and probably a seg-fault).
    /// - As a bonus tip, you are much less likely to invoke UB if you do `nn.as_ptr().read()`
    ///   instead of using something like `nn.as_ref()`. Of course, this might not be possible, but
    ///   if your type is trivially copyable (I would suggest 16-24 bytes or less), then you should
    ///   always `ptr::read` instead.
    pub fn alloc(&mut self, value: T) -> Result<NonNull<T>, T> {
        if self.len < self.capacity() {
            // SAFETY: did the exact required bounds check
            let mut ptr = unsafe { self.buf.get_unchecked(self.len) };
            // SAFETY:
            //     - valid for writes, since we have exclusive access to this memory location
            //     - aligned properly because of `RawBuf`'s layout guarantees
            unsafe {
                ptr::write(ptr.as_mut(), value);
            }
            self.len += 1;
            Ok(ptr)
        } else {
            Err(value)
        }
    }

    pub fn capacity(&self) -> usize {
        self.buf.data.len()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn resize(&mut self) {
        println!("resizing {self:?}");
        self.buf = RawBuf::new(self.capacity() * 2);
        self.len = 0;
    }

    pub fn resized(&mut self) -> &mut Self {
        self.resize();
        self
    }
}
