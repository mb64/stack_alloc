//! A simple memory source for testing

use core::ptr;

use memory_source::{MemorySource, BLOCK_ALIGN, BLOCK_SIZE};

/// A test memory source that gets memory using `libc::memalign`
#[derive(Clone, Copy, Debug)]
pub struct TestMemorySource;

unsafe impl MemorySource for TestMemorySource {
    unsafe fn get_block(&self) -> Option<ptr::NonNull<u8>> {
        debug_log!("TestMemorySource: getting memory from libc::memalign\n\0");
        let ptr = ::libc::memalign(BLOCK_ALIGN, BLOCK_SIZE);
        ptr::NonNull::new(ptr).map(ptr::NonNull::cast)
    }
}
