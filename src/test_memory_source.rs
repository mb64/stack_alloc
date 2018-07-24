//! A simple memory source for testing

use core::ptr;

use memory_source::{MemorySource, BLOCK_SIZE, BLOCK_ALIGN};

/// A test memory source that gets memory using `libc::memalign`
#[derive(Clone, Copy, Debug)]
pub struct MyGreatMemorySource;

unsafe impl MemorySource for MyGreatMemorySource {
    unsafe fn get_block() -> Option<ptr::NonNull<u8>> {
        debug_log!("MyGreatMemorySource: getting memory from libc::memalign\n\0");
        let ptr = ::libc::memalign(BLOCK_ALIGN, BLOCK_SIZE);
        ptr::NonNull::new(ptr).map(ptr::NonNull::cast)
    }
}
