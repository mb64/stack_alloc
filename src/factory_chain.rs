//! This allocator chooses from an array of `SizedAllocator`s based on the size of the allocation.
//!
//! It has a selection of static `SizedAllocator`s that it can choose from, with chunk sizes
//! ranging from 1 byte to 4 KiB.
//!
//! TODO better docs

use core::alloc::{self, Alloc, Layout};
use core::marker::PhantomData;
use core::ptr;

use bitmapped_stack::STACK_SIZE;
use memory_source::MemorySource;
use metadata_box::MetadataBox;
use sized_allocator::{SizedAllocator, DeallocResponse};

const VERY_SMALL_CHUNK_SIZE: usize = 1;
const SMALL_CHUNK_SIZE: usize = 8;
const MEDIUM_CHUNK_SIZE: usize = 64;
const LARGE_CHUNK_SIZE: usize = 512;
const VERY_LARGE_CHUNK_SIZE: usize = 4096;

const METADATA_CHUNK_SIZE: usize = 64;

/// What size allocator an allocation belongs to
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
enum SizeCategory {
    VerySmall,
    Small,
    Medium,
    Large,
    VeryLarge,
}
impl SizeCategory {
    fn choose(size: usize) -> Option<Self> {
        match size {
            0 => None,
            1..=7 => Some(SizeCategory::VerySmall),
            8..=63 => Some(SizeCategory::Small),
            64..=511 => Some(SizeCategory::Medium),
            511..=4095 => Some(SizeCategory::Large),
            4096..=262144 => Some(SizeCategory::VeryLarge),
            _ => None,
        }
    }
}

/// The `FactoryChain` buckets allocations into small (size < 64 bytes), medium (64 bytes < size <
/// 4 KiB) and large (4 KiB < size).
///
/// It has small, medium and large `SizedAllocator`s, as well as a generic `MemorySource`.
#[derive(Debug)]
pub struct FactoryChain<T: MemorySource> {
    /// 1 byte chunk size
    very_small: Option<MetadataBox<SizedAllocator>>,
    /// 8 byte chunk size
    small: Option<MetadataBox<SizedAllocator>>,
    /// 64 byte chunk size
    medium: Option<MetadataBox<SizedAllocator>>,
    /// Another 64-byte chunk size, for the metadata
    metadata: Option<MetadataBox<SizedAllocator>>,
    /// 512 byte chunk size
    large: Option<MetadataBox<SizedAllocator>>,
    /// 4 KiB chunk size
    very_large: Option<MetadataBox<SizedAllocator>>,
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
            very_small: None,
            small: None,
            medium: None,
            large: None,
            very_large: None,
            metadata: None,
            source: PhantomData,
        }
    }

    fn very_small_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.very_small.as_mut().map(|x| &mut **x)
    }
    fn small_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.small.as_mut().map(|x| &mut **x)
    }
    fn medium_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.medium.as_mut().map(|x| &mut **x)
    }
    fn metadata_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.metadata.as_mut().map(|x| &mut **x)
    }
    fn large_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.large.as_mut().map(|x| &mut **x)
    }
    fn very_large_mut(&mut self) -> Option<&mut SizedAllocator> {
        self.very_large.as_mut().map(|x| &mut **x)
    }

    unsafe fn get_very_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.very_large.is_none() {
            self.extend_very_large()
        } else {
            self.very_large_mut().ok_or(alloc::AllocErr)
        }
    }
    unsafe fn get_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.large.is_none() {
            self.extend_large()
        } else {
            self.large_mut().ok_or(alloc::AllocErr)
        }
    }
    unsafe fn get_medium(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.medium.is_none() {
            self.extend_medium()
        } else {
            self.medium_mut().ok_or(alloc::AllocErr)
        }
    }
    unsafe fn get_metadata(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.metadata.is_none() {
            self.extend_metadata()
        } else {
            self.metadata_mut().ok_or(alloc::AllocErr)
        }
    }
    unsafe fn get_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.small.is_none() {
            self.extend_small()
        } else {
            self.small_mut().ok_or(alloc::AllocErr)
        }
    }
    unsafe fn get_very_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        if self.very_small.is_none() {
            self.extend_very_small()
        } else {
            self.very_small_mut().ok_or(alloc::AllocErr)
        }
    }

    /// Returns the owner of the given pointer, or `None` if no allocator claims to own it
    fn owner_of(&mut self, _ptr: ptr::NonNull<u8>, layout: Layout) -> Option<&mut SizedAllocator> {
        match SizeCategory::choose(layout.size()) {
            Some(SizeCategory::VerySmall) => {
                debug_log!("FactoryChain: very small owns pointer %#zx\n\0", _ptr);
                debug_assert!(self.very_small.as_ref().map_or(false, |vs| vs.owns(_ptr)));
                self.very_small_mut()
            },
            Some(SizeCategory::Small) => {
                debug_log!("FactoryChain: small owns pointer %#zx\n\0", _ptr);
                debug_assert!(self.small.as_ref().map_or(false, |s| s.owns(_ptr)));
                self.small_mut()
            },
            Some(SizeCategory::Medium) => {
                debug_log!("FactoryChain: medium owns pointer %#zx\n\0", _ptr);
                debug_assert!(self.medium.as_ref().map_or(false, |m| m.owns(_ptr)));
                self.medium_mut()
            },
            Some(SizeCategory::Large) => {
                debug_log!("FactoryChain: large owns pointer %#zx\n\0", _ptr);
                debug_assert!(self.large.as_ref().map_or(false, |l| l.owns(_ptr)));
                self.large_mut()
            },
            Some(SizeCategory::VeryLarge) => {
                debug_log!("FactoryChain: very large owns pointer %#zx\n\0", _ptr);
                debug_assert!(self.very_large.as_ref().map_or(false, |vl| vl.owns(_ptr)));
                self.very_large_mut()
            },
            None => {
                debug_log!("FactoryChain: no one owns pointer %#zx!\n\0", _ptr);
                None
            },
        }
    }

    // FIXME (unimportant) these discard the entire chain of allocators on some failures
    /// Tries to add a new allocator to start of the `very_small` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_very_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let layout = Layout::from_size_align_unchecked(VERY_SMALL_CHUNK_SIZE*STACK_SIZE, VERY_SMALL_CHUNK_SIZE);
            let memory = self.alloc_medium(layout)?;
            let old_very_small = self.very_small.take();
            let new_alloc = SizedAllocator::from_memory_chunk(VERY_SMALL_CHUNK_SIZE, memory, old_very_small);
            self.store_metadata(new_alloc)?
        };
        self.very_small = Some(alloc_box);
        self.very_small_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `small` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_small(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let layout = Layout::from_size_align_unchecked(SMALL_CHUNK_SIZE*STACK_SIZE, SMALL_CHUNK_SIZE);
            let memory = self.alloc_large(layout)?;
            let old_small = self.small.take();
            let new_alloc = SizedAllocator::from_memory_chunk(SMALL_CHUNK_SIZE, memory, old_small);
            self.store_metadata(new_alloc)?
        };
        self.small = Some(alloc_box);
        self.small_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `medium` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_medium(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let layout = Layout::from_size_align_unchecked(MEDIUM_CHUNK_SIZE*STACK_SIZE, MEDIUM_CHUNK_SIZE);
            let memory = self.alloc_very_large(layout)?;
            let old_medium = self.medium.take();
            let new_alloc = SizedAllocator::from_memory_chunk(MEDIUM_CHUNK_SIZE, memory, old_medium);
            self.store_metadata(new_alloc)?
        };
        self.medium = Some(alloc_box);
        self.medium_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `metadata` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_metadata(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let (mut metadata_alloc, more_metadata) = {
                let layout = Layout::from_size_align_unchecked(METADATA_CHUNK_SIZE*STACK_SIZE, METADATA_CHUNK_SIZE);
                let (memory, more_metadata) = self.alloc_very_large_no_metadata(layout)?;
                let old_metadata = self.metadata.take();
                (SizedAllocator::from_memory_chunk(METADATA_CHUNK_SIZE, memory, old_metadata), more_metadata)
            };
            if let Some(more_metadata) = more_metadata {
                let mem = metadata_alloc.alloc(Layout::new::<SizedAllocator>())?;
                self.large = Some(MetadataBox::from_pointer_data(mem, more_metadata));
            }
            let mem = metadata_alloc.alloc(Layout::new::<SizedAllocator>())?;
            MetadataBox::from_pointer_data(mem, metadata_alloc)
        };
        self.metadata = Some(alloc_box);
        self.metadata_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `large` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let layout = Layout::from_size_align_unchecked(LARGE_CHUNK_SIZE*STACK_SIZE, LARGE_CHUNK_SIZE);
            let memory = self.alloc_very_large(layout)?;
            let old_large = self.large.take();
            let new_alloc = SizedAllocator::from_memory_chunk(LARGE_CHUNK_SIZE, memory, old_large);
            self.store_metadata(new_alloc)?
        };
        self.large = Some(alloc_box);
        self.large_mut().ok_or(alloc::AllocErr)
    }
    /// Tries to add a new allocator to start of the `large` chain.  Returns that allocator on
    /// success, `AllocErr` on failure.
    unsafe fn extend_very_large(&mut self) -> Result<&mut SizedAllocator, alloc::AllocErr> {
        let alloc_box = {
            let memory = T::get_block().ok_or(alloc::AllocErr)?;
            let old_very_large = self.very_large.take();
            let mut new_alloc = SizedAllocator::from_memory_chunk(VERY_LARGE_CHUNK_SIZE, memory, old_very_large);
            if let Some(new_alloc_place) = self.metadata.as_mut().and_then(|ma| ma.alloc(Layout::new::<SizedAllocator>()).ok()) {
                MetadataBox::from_pointer_data(new_alloc_place, new_alloc)
            } else {
                let mut metadata_alloc_box = {
                    let metadata_memory = new_alloc.alloc(Layout::from_size_align_unchecked(METADATA_CHUNK_SIZE*STACK_SIZE, METADATA_CHUNK_SIZE))?;
                    let mut metadata_alloc = SizedAllocator::from_memory_chunk(METADATA_CHUNK_SIZE, metadata_memory, None);
                    let metadata_alloc_place = metadata_alloc.alloc(Layout::new::<SizedAllocator>()).unwrap(); // unwrap bc it shouldn't fail
                    MetadataBox::from_pointer_data(metadata_alloc_place, metadata_alloc)
                };
                let new_alloc_place = metadata_alloc_box.alloc(Layout::new::<SizedAllocator>()).unwrap(); // unwrap bc it shouldn't fail
                let res = MetadataBox::from_pointer_data(new_alloc_place, new_alloc);
                self.metadata = Some(metadata_alloc_box);
                res
            }
        };
        self.very_large = Some(alloc_box);
        self.very_large_mut().ok_or(alloc::AllocErr)
    }

    /// Tries to allocate from the `very_small` chain, extending it if necessary.
    unsafe fn alloc_very_small(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= VERY_SMALL_CHUNK_SIZE*STACK_SIZE);
        match self.get_very_small()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_very_small()?.alloc(layout),
        }
    }
    /// Tries to allocate from the `small` chain, extending it if necessary.
    unsafe fn alloc_small(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= SMALL_CHUNK_SIZE*STACK_SIZE);
        match self.get_small()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_small()?.alloc(layout),
        }
    }
    /// Tries to allocate from the `medium` chain, extending it if necessary.
    unsafe fn alloc_medium(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= MEDIUM_CHUNK_SIZE*STACK_SIZE);
        match self.get_medium()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_medium()?.alloc(layout),
        }
    }
    /// Tries to allocate from the `metadata` chain, extending it if necessary.
    unsafe fn alloc_metadata(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= METADATA_CHUNK_SIZE*STACK_SIZE);
        match self.get_metadata()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_metadata()?.alloc(layout),
        }
    }
    /// Tries to allocate from the `large` chain, extending it if necessary.
    unsafe fn alloc_large(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= LARGE_CHUNK_SIZE*STACK_SIZE);
        match self.get_large()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_large()?.alloc(layout),
        }
    }
    /// Tries to allocate from the `very_large` chain, extending it if necessary.
    unsafe fn alloc_very_large(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_assert!(layout.size() <= VERY_LARGE_CHUNK_SIZE*STACK_SIZE);
        match self.get_very_large()?.alloc(layout) {
            Ok(mem) => Ok(mem),
            Err(_) => self.extend_very_large()?.alloc(layout),
        }
    }

    /// Tries to allocate from the chain that corresponds to the size category, extending it if
    /// neccessary
    unsafe fn alloc_size(&mut self, layout: Layout, size_category: SizeCategory) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        match size_category {
            SizeCategory::VerySmall => self.alloc_very_small(layout),
            SizeCategory::Small => self.alloc_small(layout),
            SizeCategory::Medium => self.alloc_medium(layout),
            SizeCategory::Large => self.alloc_large(layout),
            SizeCategory::VeryLarge => self.alloc_very_large(layout),
        }
    }

    /// Tries to allocate from the `large` chain, extending it if necessary, but doesn't store away
    /// any extra metadata created
    unsafe fn alloc_very_large_no_metadata(&mut self, layout: Layout) -> Result<(ptr::NonNull<u8>, Option<SizedAllocator>), alloc::AllocErr> {
        debug_assert!(layout.size() <= VERY_LARGE_CHUNK_SIZE*STACK_SIZE);

        if let Some(ref mut very_large) = self.very_large {
            if let Ok(mem) = very_large.alloc(layout) {
                Ok((mem, None))
            } else {
                // Extend it without storing metadata...
                let mut new_very_large = {
                    let new_mem = T::get_block().ok_or(alloc::AllocErr)?;
                    let old_very_large = self.very_large.take();
                    SizedAllocator::from_memory_chunk(VERY_LARGE_CHUNK_SIZE, new_mem, old_very_large)
                };
                if let Ok(mem) = new_very_large.alloc(layout) {
                    Ok((mem,Some(new_very_large)))
                } else {
                    // FIXME (unimportant) discards entire `very_large` chain
                    Err(alloc::AllocErr)
                }
            }
        } else {
            // Extend it without storing metadata...
            let mut new_very_large = {
                let new_mem = T::get_block().ok_or(alloc::AllocErr)?;
                let old_very_large = self.very_large.take();
                SizedAllocator::from_memory_chunk(VERY_LARGE_CHUNK_SIZE, new_mem, old_very_large)
            };
            if let Ok(mem) = new_very_large.alloc(layout) {
                Ok((mem,Some(new_very_large)))
            } else {
                // FIXME (unimportant) discards entire `large` chain
                Err(alloc::AllocErr)
            }
        }
    }

    unsafe fn store_metadata(&mut self, alloc: SizedAllocator) -> Result<MetadataBox<SizedAllocator>, alloc::AllocErr> {
        let layout: Layout = Layout::new::<SizedAllocator>();
        self.alloc_metadata(layout)
            .map(|ptr| MetadataBox::from_pointer_data(ptr, alloc))
    }
}

unsafe impl<T: MemorySource> Alloc for FactoryChain<T> {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_log!("FactoryChain: allocating size %zu align %zu\n\0", layout.size(), layout.align());
        if let Some(category) = SizeCategory::choose(layout.size()) {
            self.alloc_size(layout, category)
        } else {
            Err(alloc::AllocErr)
        }
    }

    unsafe fn dealloc(&mut self, ptr: ptr::NonNull<u8>, layout: Layout) {
        if layout.size() == 0 {
            return;
        }
        let owner = self.owner_of(ptr, layout).expect("No allocator owns the memory to deallocate");
        if let DeallocResponse::FreeAllocator(allocator) = owner.dealloc(ptr, layout) {
            let stack_layout = {
                let size = allocator.chunk_size() * STACK_SIZE;
                let layout = allocator.chunk_size();
                Layout::from_size_align_unchecked(size, layout)
            };
            let stack_ptr = allocator.stack_pointer();
            self.dealloc(stack_ptr, stack_layout);
            self.metadata.as_mut().unwrap().dealloc(allocator.into_raw().cast(), Layout::new::<SizedAllocator>());
        }
    }

    unsafe fn realloc(&mut self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        let new_layout = Layout::from_size_align_unchecked(new_size, layout.align());

        // Try to expand it in place if the size category hasn't changed
        if SizeCategory::choose(layout.size()) == SizeCategory::choose(new_size) {
            let alloc = self.owner_of(ptr, layout).expect("No allocator owns the memory to realloc");
            if new_size <= layout.size() {
                alloc.shrink_in_place(ptr, layout, new_size);
                return Ok(ptr);
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
