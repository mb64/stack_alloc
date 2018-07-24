//! A simple global allocator for testing

use alloc::alloc::{Layout, GlobalAlloc};
use core::ptr;

use std::sync::atomic::{AtomicBool, Ordering};

use factory_chain::FactoryChain;
use memory_source::{MemorySource, BLOCK_SIZE, BLOCK_ALIGN};

/// A simple allocator for testing purposes
#[derive(Clone, Copy, Debug)]
pub struct SimpleTestAlloc;

#[derive(Clone, Copy, Debug)]
pub struct MyGreatMemorySource;

unsafe impl MemorySource for MyGreatMemorySource {
    unsafe fn get_block() -> Option<ptr::NonNull<u8>> {
        debug_log!("MyGreatMemorySource: getting memory from libc::memalign\n\0");
        let ptr = ::libc::memalign(BLOCK_ALIGN, BLOCK_SIZE);
        ptr::NonNull::new(ptr).map(ptr::NonNull::cast)
    }
}
/*
static ALLOC: FactoryChain<MyGreatMemorySource> = FactoryChain::new();
static IN_USE: AtomicBool = AtomicBool::new(false);

struct Lock;
impl Lock {
    fn get() -> Lock {
        let mut spinning = false;
        while IN_USE.swap(true, Ordering::SeqCst) == true {
            if !spinning {
                debug_log!("Spinning...\n\0");
                spinning = true;
            }
        }
        Lock
    }
}
impl Drop for Lock {
    fn drop(&mut self) {
        assert!(IN_USE.swap(false, Ordering::SeqCst));
    }
}

unsafe impl GlobalAlloc for SimpleTestAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _l = Lock::get();
        debug_log!("SimpleTestAlloc: allocating size %zu align %zu\n\0", layout.size(), layout.align());
        let ptr = ALLOC.alloc(layout);
        debug_log!("SimpleTestAlloc: done allocating pointer %#zx\n\n\0", ptr);
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _l = Lock::get();
        debug_log!("SimpleTestAlloc: deallocating size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr);
        ALLOC.dealloc(ptr, layout);
        debug_log!("SimpleTestAlloc: done deallocating pointer %#zx\n\n\0", ptr);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let _l = Lock::get();
        debug_log!("SimpleTestAlloc: reallocating size %zu to %zu align %zu pointer %#zx\n\0", layout.size(), new_size, layout.align(), ptr);
        let new_ptr = ALLOC.realloc(ptr, layout, new_size);
        debug_log!("SimpleTestAlloc: done reallocating pointer %#zx to new pointer %#zx\n\n\0", ptr, new_ptr);
        new_ptr
    }
}
*/
