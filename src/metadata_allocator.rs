//! You need somewhere to put the metadata for your allocators
//!
//! That place is here

use core::mem;

use sized_allocator::SizedAllocator;

const CHUNK_ARRAY_LEN: usize = mem::size_of::<SizedAllocator>() / mem::size_of::<u64>() + 1;

/// A metadata chunk.
///
/// One chunk has the same size as a unit of metadata, but it contains zeros
#[derive(Copy, Clone, Default, Debug)]
struct Chunk {
    _fake_data: [u64; CHUNK_ARRAY_LEN],
}
impl Chunk {
    const fn new() -> Self {
        Chunk {
            _fake_data: [0; CHUNK_ARRAY_LEN],
        }
    }
}


fn move_into(alloc: SizedAllocator, chunk: &mut Chunk) -> &mut SizedAllocator {
    unsafe {
        let chunk_ptr: *mut Chunk = chunk;
        let ptr = chunk_ptr as *mut SizedAllocator;
        ptr.write(alloc);
        ptr.as_mut().unwrap()
    }
}

const STACK_SIZE: usize = 64;

static mut STACK: [Chunk; STACK_SIZE] = [Chunk::new(); 64];
static mut STACK_HEIGHT: usize = 0;

/// Stores the metadata in the metadata stack.
pub fn store_metadata(alloc: SizedAllocator) -> &'static SizedAllocator {
    let chunk = unsafe {
        let reserved_place = STACK_HEIGHT;
        STACK_HEIGHT += 1;
        assert!(STACK_HEIGHT < STACK_SIZE);
        &mut STACK[reserved_place]
    };
    move_into(alloc, chunk)
}
