//! This allocator chooses from an array of `SizedAllocator`s based on the size of the allocation.
//!
//! It has a selection of static `SizedAllocator`s that it can choose from, with chunk sizes
//! ranging from 1 byte to 16 KiB.
//!
//! TODO better docs

use core::alloc::{GlobalAlloc, Layout};
use core::cell::Cell;
use core::marker::PhantomData;
use core::ptr;

use sized_allocator::SizedAllocator;
use memory_source::MemorySource;
use metadata_allocator;

const LARGE_CHUNK_SIZE: usize = 4096;
const MEDIUM_CHUNK_SIZE: usize = 64;
const SMALL_CHUNK_SIZE: usize = 1;

/// The `FactoryChain` buckets allocations into small (size < 64 bytes), medium (64 bytes < size <
/// 4 KiB) and large (4 KiB < size).
///
/// It has small, medium and large `SizedAllocator`s, as well as a generic `MemorySource`.
#[derive(Debug)]
pub struct FactoryChain<T: MemorySource> {
    /// 1 byte chunk size
    small: Cell<Option<&'static SizedAllocator>>,
    /// 64 byte chunk size
    medium: Cell<Option<&'static SizedAllocator>>,
    /// 16 KiB chunk size
    large: Cell<Option<&'static SizedAllocator>>,
    /// Gives 256 KiB chunks
    source: PhantomData<T>,
}

//TODO what
//impl<T> !Unpin for FactoryChain<T> {}

impl<T: MemorySource> Default for FactoryChain<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: MemorySource> FactoryChain<T> {
    /// Creates a new `FactoryChain<T>`, without allocating any memory.
    ///
    /// The first allocation with get a block from the memory source an initialize the necessary
    /// allocators.
    pub const fn new() -> Self {
        FactoryChain {
            small: Cell::new(None),
            medium: Cell::new(None),
            large: Cell::new(None),
            source: PhantomData,
        }
    }

    fn get_large(&self) -> Option<&'static SizedAllocator> {
        self.large.update(|prev| prev.or_else(|| {
            let large_alloc = SizedAllocator::from_memory_source::<T>(LARGE_CHUNK_SIZE)?;
            Some(metadata_allocator::store_metadata(large_alloc))
        }))
    }
    fn get_medium(&self) -> Option<&'static SizedAllocator> {
        self.medium.update(|prev| prev.or_else(|| {
            self.get_large().map(|large| {
                let medium_alloc = SizedAllocator::from_sized_alloc_factory(large, MEDIUM_CHUNK_SIZE);
                metadata_allocator::store_metadata(medium_alloc)
            })
        }))
    }
    fn get_small(&self) -> Option<&'static SizedAllocator> {
        self.small.update(|prev| prev.or_else(|| {
            self.get_medium().map(|medium| {
                let small_alloc = SizedAllocator::from_sized_alloc_factory(medium, SMALL_CHUNK_SIZE);
                metadata_allocator::store_metadata(small_alloc)
            })
        }))
    }

    /// Returns the owner of the given pointer, or `None` if no allocator claims to own it
    fn owner_of(&self, ptr: *mut u8) -> Option<&'static SizedAllocator> {
        if let Some(small) = self.small.get().filter(|small| small.owns(ptr)) {
            Some(small)
        } else if let Some(medium) = self.medium.get().filter(|medium| medium.owns(ptr)) {
            Some(medium)
        } else if let Some(large) = self.large.get().filter(|large| large.owns(ptr)) {
            Some(large)
        } else {
            None
        }
    }
}


unsafe impl<T: MemorySource> GlobalAlloc for FactoryChain<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match layout.size() {
            0       => ptr::null_mut(),
            1 ..=49 => {
                if let Some(small) = self.get_small() {
                    small.alloc(layout)
                } else {
                    ptr::null_mut()
                }
            },
            50..=3_499 => {
                if let Some(medium) = self.get_medium() {
                    medium.alloc(layout)
                } else {
                    ptr::null_mut()
                }
            },
            3_500..=262_144 => {
                if let Some(large) = self.get_large() {
                    large.alloc(layout)
                } else {
                    ptr::null_mut()
                }
            },
            _ => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        self.owner_of(ptr).expect("No allocator owns the memory to deallocate").dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        let nonnull = match ptr::NonNull::new(ptr) {
            Some(x) => x,
            None => return self.alloc(new_layout),
        };
        let alloc = self.owner_of(ptr).expect("No allocator claims to own the memory");
        if new_size <= layout.size() {
            if alloc.shrink_in_place(nonnull, layout, new_size).is_ok() {
                return ptr;
            }
        } else {
            if alloc.grow_in_place(nonnull, layout, new_size).is_ok() {
                return ptr;
            }
        }

        // Because changing it in place didn't work, just get new memory
        let new_memory = self.alloc(new_layout);
        if !new_memory.is_null() {
            ptr::copy_nonoverlapping(
                ptr,
                new_memory,
                ::core::cmp::min(layout.size(), new_size));
        }
        new_memory
    }
}
