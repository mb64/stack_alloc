/// The `SizedAllocator`s are in linked lists of allocators.
///
/// This is the indirection between them.

//use core::marker::PhantomData;
use core::ptr;
use core::ops;

// TODO Docs
#[derive(Debug)]
pub struct MetadataBox<T> {
    ptr: ptr::NonNull<T>,
    //phantom: PhantomData<T>,
}

impl<T> ops::Deref for MetadataBox<T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe {
            self.ptr.as_ref()
        }
    }
}
impl<T> ops::DerefMut for MetadataBox<T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe {
            self.ptr.as_mut()
        }
    }
}

impl<T> MetadataBox<T> {
    /// Create a `MetadataBox` from the pointer to uninitialized memory and the value.
    ///
    /// It's unsafe because it assumes the pointer is a valid pointer for the type, and that it
    /// won't be deallocated while the box is still alive.
    pub unsafe fn from_pointer_data(ptr: ptr::NonNull<u8>, data: T) -> Self {
        let ptr: ptr::NonNull<T> = ptr.cast();
        ptr.as_ptr().write(data);
        Self::from_raw(ptr)
    }

    /// Creates a new `MetadataBox` from the raw pointer.
    ///
    /// It is unsafe because it assumes the pointer is a valid pointer to the right type.
    pub const unsafe fn from_raw(ptr: ptr::NonNull<T>) -> Self {
        Self { ptr }
    }

    /// Returns a pointer to the data in the box.
    pub fn into_raw(self) -> ptr::NonNull<T> {
        self.ptr
    }
}
