//! This allocator chooses from an array of `SizedAllocator`s based on the size of the allocation.
//!
//! It has a selection of static `SizedAllocator`s that it can choose from, with chunk sizes
//! ranging from 1 byte to 4 KiB.
//!
//! TODO better docs

use core::alloc::{self, Alloc, Layout};
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
    small: Option<&'static mut SizedAllocator>,
    /// 64 byte chunk size
    medium: Option<&'static mut SizedAllocator>,
    /// 4 KiB chunk size
    large: Option<&'static mut SizedAllocator>,
    /// Gives 256 KiB chunks
    source: PhantomData<T>,
}

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
            small: None,
            medium: None,
            large: None,
            source: PhantomData,
        }
    }

    fn small_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.small.as_mut().map(|x| &mut **x)
    }
    fn medium_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.medium.as_mut().map(|x| &mut **x)
    }
    fn large_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.large.as_mut().map(|x| &mut **x)
    }

    unsafe fn get_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.large.is_none() {
            let large_alloc = SizedAllocator::from_memory_source::<T>(LARGE_CHUNK_SIZE, None).ok_or(alloc::AllocErr)?;
            self.large = Some(metadata_allocator::store_metadata(large_alloc));
        }
        self.large_mut().ok_or(alloc::AllocErr)
    }
    unsafe fn get_medium(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.medium.is_none() {
            self.medium = self.get_large().ok().and_then(|large| {
                let medium_alloc = SizedAllocator::from_sized_alloc_factory(MEDIUM_CHUNK_SIZE, large, None)?;
                Some(metadata_allocator::store_metadata(medium_alloc))
            });
        }
        self.medium_mut().ok_or(alloc::AllocErr)
    }
    unsafe fn get_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.small.is_none() {
            self.small = self.get_medium().ok().and_then(|medium| {
                let small_alloc = SizedAllocator::from_sized_alloc_factory(SMALL_CHUNK_SIZE, medium, None)?;
                Some(metadata_allocator::store_metadata(small_alloc))
            });
        }
        self.small_mut().ok_or(alloc::AllocErr)
    }

    /// Returns the owner of the given pointer, or `None` if no allocator claims to own it
    fn owner_of(&mut self, ptr: ptr::NonNull<u8>) -> Option<&mut SizedAllocator> {
        let raw_ptr = ptr.as_ptr();
        if let Some(small) = self.small.as_mut().filter(|small| small.owns(raw_ptr)) {
            debug_log!("FactoryChain: small owns pointer %#zx\n\0", raw_ptr);
            Some(small)
        } else if let Some(medium) = self.medium.as_mut().filter(|medium| medium.owns(raw_ptr)) {
            debug_log!("FactoryChain: medium owns pointer %#zx\n\0", raw_ptr);
            Some(medium)
        } else if let Some(large) = self.large.as_mut().filter(|large| large.owns(raw_ptr)) {
            debug_log!("FactoryChain: large owns pointer %#zx\n\0", raw_ptr);
            Some(large)
        } else {
            debug_log!("FactoryChain: no one owns pointer %#zx!\n\0", raw_ptr);
            None
        }
    }

    /// Tries to add a new allocator to start of the `small` chain.  Returns that allocator on
    /// success, `None` on failure.
    unsafe fn extend_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_ref = {
            let old_small = self.small.take();
            let new_alloc = SizedAllocator::from_sized_alloc_factory(SMALL_CHUNK_SIZE, self.get_medium()?, old_small).ok_or(alloc::AllocErr)?;
            metadata_allocator::store_metadata(new_alloc)
        };
        self.small = Some(alloc_ref);
        self.small_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `medium` chain.  Returns that allocator on
    /// success, `None` on failure.
    unsafe fn extend_medium(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_ref = {
            let old_medium = self.medium.take();
            let new_alloc = SizedAllocator::from_sized_alloc_factory(MEDIUM_CHUNK_SIZE, self.get_large()?, old_medium).ok_or(alloc::AllocErr)?;
            metadata_allocator::store_metadata(new_alloc)
        };
        self.medium = Some(alloc_ref);
        self.medium_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `large` chain.  Returns that allocator on
    /// success, `None` on failure.
    unsafe fn extend_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_ref = {
            let old_large = self.large.take();
            let mut new_alloc = SizedAllocator::from_memory_source::<T>(LARGE_CHUNK_SIZE, old_large).ok_or(alloc::AllocErr)?;
            metadata_allocator::store_metadata(new_alloc)
        };
        self.large = Some(alloc_ref);
        self.large_mut().ok_or(alloc::AllocErr)
    }
}

unsafe impl<T: MemorySource> Alloc for FactoryChain<T> {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_log!("FactoryChain: delegating allocation (size %zu align %zu) to \0", layout.size(), layout.align());
        match layout.size() {
            0       => Err(alloc::AllocErr),
            1 ..=47 => {
                debug_log!("small\n\0");
                if let Ok(mem) = self.get_small()?.alloc(layout) {
                    Ok(mem)
                } else {
                    self.extend_small()?.alloc(layout)
                }
            },
            48..=3_499 => {
                debug_log!("medium\n\0");
                if let Ok(mem) = self.get_medium()?.alloc(layout) {
                    Ok(mem)
                } else {
                    self.extend_medium()?.alloc(layout)
                }
            },
            3_500..=262_144 => {
                debug_log!("large\n\0");
                if let Ok(mem) = self.get_large()?.alloc(layout) {
                    Ok(mem)
                } else {
                    self.extend_large()?.alloc(layout)
                }
            },
            _ => Err(alloc::AllocErr),
        }
    }

    unsafe fn dealloc(&mut self, ptr: ptr::NonNull<u8>, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        self.owner_of(ptr).expect("No allocator owns the memory to deallocate").dealloc(ptr, layout);
    }

    unsafe fn realloc(&mut self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        {
            let alloc = self.owner_of(ptr).expect("No allocator claims to own the memory");
            if new_size <= layout.size() {
                if alloc.shrink_in_place(ptr, layout, new_size).is_ok() {
                    return Ok(ptr);
                }
            } else {
                if alloc.grow_in_place(ptr, layout, new_size).is_ok() {
                    return Ok(ptr);
                }
            }
        }

        // Because changing it in place didn't work, just get new memory
        let new_memory = self.alloc(new_layout);
        if let Ok(new_memory) = new_memory {
            ptr::copy_nonoverlapping(
                ptr.as_ptr(),
                new_memory.as_ptr(),
                ::core::cmp::min(layout.size(), new_size));
            self.dealloc(ptr, layout);
        }
        new_memory
    }
}
