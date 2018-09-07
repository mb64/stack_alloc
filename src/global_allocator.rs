//! The `Allocator` type

use core::alloc::{Alloc, GlobalAlloc, Layout};
use core::cell;
use core::ops;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use bucketed::{BucketedAllocator, Buckets};
use memory_source::MemorySource;

/// The `Allocator` type is the way to set up a global allocator.  It implements the
/// `std::alloc::GlobalAlloc` trait, allowing it to be used as the allocator.
///
/// For how to use it in your program, see the crate docs.  For how it works, see the `README.md`
/// file.
///
/// See the [`std`
/// docs](https://doc.rust-lang.org/nightly/std/alloc/index.html#the-global_allocator-attribute)
/// for more information on global allocators.
#[derive(Debug)]
pub struct Allocator<T: MemorySource>(pub T);

/// The real behind-the-scenes allocator.
/// It has a global lock over everything.
#[derive(Debug)]
struct LockedAllocator {
    alloc: cell::UnsafeCell<Buckets>,
    lock: AtomicBool,
}

static GLOBAL_LOCKED_ALLOCATOR: LockedAllocator = LockedAllocator {
    alloc: cell::UnsafeCell::new(Buckets::new()),
    lock: AtomicBool::new(false),
};

unsafe impl Sync for LockedAllocator {}

#[derive(Debug)]
struct Lock<'a>(&'a LockedAllocator);

impl<'a> Drop for Lock<'a> {
    fn drop(&mut self) {
        let prev = self.0.lock.swap(false, Ordering::SeqCst);
        debug_assert_eq!(prev, true);
    }
}

impl<'a> ops::Deref for Lock<'a> {
    type Target = Buckets;

    fn deref(&self) -> &Buckets {
        unsafe { &*self.0.alloc.get() }
    }
}
impl<'a> ops::DerefMut for Lock<'a> {
    fn deref_mut(&mut self) -> &mut Buckets {
        unsafe { &mut *self.0.alloc.get() }
    }
}

impl LockedAllocator {
    fn get_buckets(&self) -> Lock<'_> {
        let mut spinning = false;
        while self.lock.swap(true, Ordering::SeqCst) == true {
            if !spinning {
                spinning = true;
                debug_log!("Spinning...\n\0");
            }
        }
        Lock(self)
    }
}

impl<S: MemorySource> Allocator<S> {
    fn get_alloc(&self) -> BucketedAllocator<'_, Lock<'_>, S> {
        BucketedAllocator::new(GLOBAL_LOCKED_ALLOCATOR.get_buckets(), &self.0)
    }
}

fn to_raw<E>(ptr: Result<ptr::NonNull<u8>, E>) -> *mut u8 {
    match ptr {
        Ok(nonnull) => nonnull.as_ptr(),
        _ => ptr::null_mut(),
    }
}

unsafe impl<T: MemorySource> GlobalAlloc for Allocator<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_log!(
            "Allocator: allocating size %zu align %zu\n\0",
            layout.size(),
            layout.align()
        );
        let ptr = if layout.size() == 0 {
            ptr::null_mut()
        } else {
            to_raw(self.get_alloc().alloc(layout))
        };
        debug_log!("Allocator: done allocating pointer %#zx\n\n\0", ptr);
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_log!(
            "Allocator: deallocating size %zu align %zu pointer %#zx\n\0",
            layout.size(),
            layout.align(),
            ptr
        );
        if let Some(nonnull) = ptr::NonNull::new(ptr) {
            self.get_alloc().dealloc(nonnull, layout);
        }
        debug_log!("Allocator: done deallocating pointer %#zx\n\n\0", ptr);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        debug_log!(
            "Allocator: reallocating size %zu to %zu align %zu pointer %#zx\n\0",
            layout.size(),
            new_size,
            layout.align(),
            ptr
        );
        let new_ptr = if let Some(nonnull) = ptr::NonNull::new(ptr) {
            self.get_alloc().realloc(nonnull, layout, new_size)
        } else {
            self.get_alloc()
                .alloc(Layout::from_size_align_unchecked(new_size, layout.align()))
        };
        let new_ptr = to_raw(new_ptr);
        debug_log!(
            "Allocator: done reallocating pointer %#zx to new pointer %#zx\n\n\0",
            ptr,
            new_ptr
        );
        new_ptr
    }
}
