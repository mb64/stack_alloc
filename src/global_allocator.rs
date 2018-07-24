//! The user-visible allocator
//!
//! ```rust
//! #![feature(const_fn)]
//!
//! #[global_allocator]
//! static GLOBAL: Allocator<TODO> = Allocator::new()
//! ```

use core::alloc::{GlobalAlloc, Layout};
use core::ops::Deref;
use core::sync::atomic::{AtomicBool, Ordering};

use factory_chain::FactoryChain;
use memory_source::MemorySource;

// TODO Docs
#[derive(Debug)]
pub struct Allocator<T: MemorySource> {
    alloc: FactoryChain<T>,
    lock: AtomicBool,
}

impl<T: MemorySource> Allocator<T> {
    pub const fn new() -> Self {
        Allocator {
            alloc: FactoryChain::new(),
            lock: AtomicBool::new(false),
        }
    }
}

unsafe impl<T: MemorySource> Sync for Allocator<T> {}

#[derive(Debug)]
struct Lock<'a, T: MemorySource + 'a>(&'a Allocator<T>);

impl<'a, T: MemorySource + 'a> Drop for Lock<'a, T> {
    fn drop(&mut self) {
        debug_assert!(self.0.lock.swap(false, Ordering::SeqCst));
    }
}

impl<'a, T: MemorySource + 'a> Deref for Lock<'a, T> {
    type Target = FactoryChain<T>;

    fn deref(&self) -> &FactoryChain<T> {
        &self.0.alloc
    }
}

impl<T: MemorySource> Allocator<T> {
    fn get_alloc(&self) -> Lock<T> {
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

unsafe impl<T: MemorySource> GlobalAlloc for Allocator<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_log!("Allocator: allocating size %zu align %zu\n\0", layout.size(), layout.align());
        let ptr = self.get_alloc().alloc(layout);
        debug_log!("Allocator: done allocating pointer %#zx\n\n\0", ptr);
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_log!("Allocator: deallocating size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr);
        self.get_alloc().dealloc(ptr, layout);
        debug_log!("Allocator: done deallocating pointer %#zx\n\n\0", ptr);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        debug_log!("Allocator: reallocating size %zu to %zu align %zu pointer %#zx\n\0", layout.size(), new_size, layout.align(), ptr);
        let new_ptr = self.get_alloc().realloc(ptr, layout, new_size);
        debug_log!("Allocator: done reallocating pointer %#zx to new pointer %#zx\n\n\0", ptr, new_ptr);
        new_ptr
    }
}
