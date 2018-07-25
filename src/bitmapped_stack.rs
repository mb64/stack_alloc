//! A stack allocator with a bitmap as well.
//!
//! This way it can actually de-allocate things.

use alloc::alloc::{self, Layout, AllocErr};
use core::ptr::NonNull;
use core::ops;

/// The size, in chunks, of each bitmapped stack
pub const STACK_SIZE: usize = 64;

/// Rounds the given number up to fit the alignment.
/// `alignment` must be a power of 2.
fn round_up_to_alignment(x: usize, alignment: usize) -> usize {
    debug_assert_ne!(alignment, 0);
    let alignment_mask = alignment - 1;
    if x & alignment_mask != 0 {
        (x + alignment) & (!alignment_mask)
    } else {
        x
    }
}

/// An upwards-growing stack
#[derive(Debug)]
pub struct BitmappedStack {
    /// The bottom of the stack
    bottom: NonNull<u8>,
    /// Measured in units of `chunk_size`
    current_height: usize,
    /// Measured in bytes
    chunk_size: usize,
    /// Each bit is one chunk
    bitmap: u64,
}

impl BitmappedStack {
    /// Returns a new `BitmappedStack`.  Panics if total_size > 64
    pub const fn new(pointer: NonNull<u8>, chunk_size: usize) -> Self {
        BitmappedStack {
            bottom: pointer,
            current_height: 0,
            chunk_size,
            bitmap: 0x0000000000000000,
        }
    }

    /// Returns whether or not this allocator owns this memory
    pub fn owns(&self, pointer: *const u8) -> bool {
        let addr = pointer as usize;
        let min = self.chunk_to_ptr(0).as_ptr() as usize;
        let max = self.chunk_to_ptr(STACK_SIZE - 1).as_ptr() as usize;
        min <= addr && addr <= max
    }

    /// Returns the smallest allocation size of the stack
    pub fn chunk_size(&self) -> usize {
        self.chunk_size
    }

    /// Returns a pointer to the bottom of the stack
    pub fn pointer(&self) -> NonNull<u8> {
        self.bottom
    }

    /// Returns true iff there are no allocations on the stack
    pub fn is_empty(&self) -> bool {
        self.current_height == 0
    }

    /// For debug purposes, `debug_assert`s that the allocator completely deallocated
    pub fn debug_assert_empty(&self) {
        debug_assert_eq!(self.bitmap, 0, "The mask is not zero :(");
        debug_assert_eq!(self.current_height, 0, "The height is not zero :(");
    }

    /// For debug purposes, `debug_assert`s that the allocator has made allocations
    pub fn debug_assert_nonempty(&self) {
        debug_assert_ne!(self.bitmap, 0, "The bitmap is zero :(");
        debug_assert_ne!(self.current_height, 0, "The height is zero :(");
    }

    /// Returns the number of chunks required for the given number of bytes
    fn chunks_for(&self, bytes: usize) -> usize {
        // Divide by chunk size, rounding up
        let mut res = bytes / self.chunk_size;
        if bytes % self.chunk_size != 0 {
            res += 1;
        }
        res
    }

    /// Mark the chunks as allocated in the bitmap
    unsafe fn bitmap_allocate(&mut self, chunk_range: ops::Range<usize>) {
        debug_assert!(chunk_range.end <= STACK_SIZE);
        let mask = {
            let num_chunks = chunk_range.size_hint().0;
            // Wrapping ops in case of (1 << 64) - 1 which overflows
            let mask_base = 1_u64.wrapping_shl(num_chunks as u32).wrapping_sub(1);
            mask_base << chunk_range.start
        };

        self.bitmap |= mask;
    }

    /// Mark the chunks as deallocated in the bitmap
    unsafe fn bitmap_deallocate(&mut self, chunk_range: ops::Range<usize>) {
        debug_assert!(chunk_range.end <= STACK_SIZE);
        let mask = {
            let num_chunks = chunk_range.size_hint().0;
            // Wrapping ops in case of (1 << 64) - 1 which overflows
            let mask_base = 1_u64.wrapping_shl(num_chunks as u32).wrapping_sub(1);
            mask_base << chunk_range.start
        };

        self.bitmap &= !mask;
    }

    /// Returns `true` iff the chunk is marked as allocated by the bitmap
    fn is_chunk_allocated(&self, chunk: usize) -> bool {
        debug_assert!(chunk < STACK_SIZE, "chunk {} out of bounds", chunk);
        self.bitmap & (1 << chunk) != 0
    }

    /// Returns `true` if all the chunks in the range are marked as allocated in the bitmap
    fn all_allocated(&self, chunk_range: ops::Range<usize>) -> bool {
        debug_assert!(chunk_range.end <= STACK_SIZE);
        let mask = {
            let num_chunks = chunk_range.size_hint().0;
            let mask_base = (1 << num_chunks) - 1;
            mask_base << chunk_range.start
        };

        self.bitmap | (!mask) == !0
    }

    /// Returns `true` if all the chunks in the range are marked as deallocated in the bitmap
    fn all_deallocated(&self, chunk_range: ops::Range<usize>) -> bool {
        debug_assert!(chunk_range.end <= STACK_SIZE);
        let mask = {
            let num_chunks = chunk_range.size_hint().0;
            let mask_base = (1 << num_chunks) - 1;
            mask_base << chunk_range.start
        };

        self.bitmap & mask == 0
    }

    /// Returns the chunk number associated with the pointer.
    ///
    /// It's unsafe because it assumes that the pointer is a valid pointer to a chunk.
    unsafe fn ptr_to_chunk(&self, ptr: *mut u8) -> usize {
        let offset = ptr.offset_from(self.bottom.as_ptr());
        debug_assert!(offset >= 0);
        offset as usize / self.chunk_size
    }

    /// Return a pointer to the chunk at that number.
    fn chunk_to_ptr(&self, chunk: usize) -> NonNull<u8> {
        // Might want to look at ptr for out-of-bounds chunks, too...?
        //debug_assert!(chunk < STACK_SIZE, "chunk {} out of bounds", chunk);
        unsafe {
            let byte_offset = chunk * self.chunk_size;
            NonNull::new_unchecked(self.bottom.as_ptr().add(byte_offset))
        }
    }

    /// Lowers the height past as many deallocated chunks as possible
    fn shrink_height(&mut self) {
        self.current_height = 64 - self.bitmap.leading_zeros() as usize;
    }

    pub unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocErr> {
        debug_log!("Allocing: align %zu, size %zu\n\0", layout.align(), layout.size());
        let bottom_of_alloc = {
            let stack_ptr = self.chunk_to_ptr(self.current_height);
            let aligned_stack_ptr = round_up_to_alignment(stack_ptr.as_ptr() as usize, layout.align());
            self.ptr_to_chunk(aligned_stack_ptr as *mut u8)
        };

        if bottom_of_alloc*self.chunk_size + layout.size() > STACK_SIZE*self.chunk_size {
            debug_log!("Exhausted BitmappedStack:\n  chunk_size: %zu\n  current_height: %zu\n  bitmap: %#018zx\n\0",
                self.chunk_size,
                self.current_height,
                self.bitmap
                );
            return Err(AllocErr);
        }

        let new_height = bottom_of_alloc + self.chunks_for(layout.size());
        self.bitmap_allocate(bottom_of_alloc..new_height);
        self.current_height = new_height;
        debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
        Ok(self.chunk_to_ptr(bottom_of_alloc))
    }

    pub unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        debug_log!("Freeing: align %zu, size %zu\n\0", layout.align(), layout.size());
        debug_assert!(self.owns(ptr.as_ptr()));
        let start_chunk = self.ptr_to_chunk(ptr.as_ptr());
        let end_chunk = start_chunk + self.chunks_for(layout.size());
        self.bitmap_deallocate(start_chunk..end_chunk);
        if self.current_height == end_chunk {
            self.current_height = start_chunk;
            self.shrink_height();
        }
        debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
    }
    
    pub unsafe fn shrink_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) {
        debug_log!("Shrinking: align %zu, size %zu to %zu\n\0", layout.align(), layout.size(), new_size);
        let new_chunks = self.chunks_for(new_size);
        let old_chunks = self.chunks_for(layout.size());
        let new_end = self.ptr_to_chunk(ptr.as_ptr()) + new_chunks;
        let old_end = self.ptr_to_chunk(ptr.as_ptr()) + old_chunks;
        self.bitmap_deallocate(new_end .. old_end);
        if self.current_height == old_end {
            self.current_height = new_end;
            if new_size == 0 {
                self.shrink_height();
            }
        }
        debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
    }

    pub unsafe fn grow_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        debug_log!("Growing: align %zu, size %zu to %zu\n\0", layout.align(), layout.size(), new_size);
        let new_chunks = self.chunks_for(new_size);
        let old_chunks = self.chunks_for(layout.size());
        let new_end = self.ptr_to_chunk(ptr.as_ptr()) + new_chunks;
        let old_end = self.ptr_to_chunk(ptr.as_ptr()) + old_chunks;
        if old_end == new_end {
            debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
            return Ok(());
        }
        if new_end > STACK_SIZE {
            return Err(alloc::CannotReallocInPlace);
        }
        debug_assert!(old_end < new_end);
        if old_end == self.current_height {
            self.current_height = new_end;
            self.bitmap_allocate(old_end..new_end);
            debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
            Ok(())
        } else {
            if self.all_deallocated(old_end..new_end) {
                self.bitmap_allocate(old_end..new_end);
                debug_log!("    Bitmap is now %#018jx\n\0", self.bitmap);
                Ok(())
            } else {
                Err(alloc::CannotReallocInPlace)
            }
        }
    }
}
