//! This module implements the method for managing linked lists of stacks of a consistent size.

use core::alloc::{self, Layout};
use core::ptr::NonNull;

use bitmapped_stack::{BitmappedStack, STACK_SIZE};
use memory_source::{self, MemorySource};

/// The recommended action after deallocating
pub enum DeallocResponse {
    /// Do nothing; everything's good
    Nothing,

    /// Remove this allocator and free its stack because it's empty
    ///
    /// It should be replaced by its backup allocator, if any
    Collapse,

    /// The given allocator should be freed; both the metadata and its stack.
    ///
    /// This happens when another allocator down the line was collapsed and its memory needs to be
    /// freed.
    FreeAllocator(&'static mut SizedAllocator),
}

/// The `SizedAllocator` type is the type that implements `GlobalAlloc`
///
/// It uses internal mutability to make it work
///
/// A `SizedAllocator` is a linked list of stacks whose chunk size is the same.
#[derive(Debug)]
pub struct SizedAllocator {
    primary: BitmappedStack,
    /// The backup allocator should have the same size chunk as the primary allocator
    backup: Option<&'static mut SizedAllocator>,
}

impl SizedAllocator {
    /// Allocates memory from the factory and creates a new `SizedAllocator` using the given chunk
    /// size.
    ///
    /// It panics if it can't allocate memory from the factory
    pub unsafe fn from_sized_alloc_factory(chunk_size: usize, factory: &mut SizedAllocator, backup: Option<&'static mut SizedAllocator>) -> Option<Self> {
        let memory = {
            let size = STACK_SIZE * chunk_size;
            let align = chunk_size;
            debug_assert_eq!(size, factory.chunk_size());
            factory.alloc(Layout::from_size_align(size, align).unwrap()).ok()?
        };

        Some(
            SizedAllocator {
                primary: BitmappedStack::new(memory, chunk_size),
                backup: backup,
            })
    }

    /// Makes a new `SizedAllocator` that uses the `MemorySource` as its factory
    pub unsafe fn from_memory_source<T: MemorySource>(chunk_size: usize, backup: Option<&'static mut SizedAllocator>) -> Option<Self> {
        debug_assert_eq!(chunk_size * 64, memory_source::BLOCK_SIZE);
        let memory = T::get_block()?;
        Some(
            SizedAllocator {
                primary: BitmappedStack::new(memory, chunk_size),
                backup: backup,
            })
    }

    /// Returns the smallest size allocation possible
    pub fn chunk_size(&self) -> usize {
        let size = self.primary.chunk_size();

        #[cfg(debug_asserts)]
        {
            if let Some(backup) = self.backup {
                debug_assert_eq!(size, backup.chunk_size());
            }
        }

        size
    }

    /// Returns a pointer to the bottom of the stack
    pub fn stack_pointer(&self) -> NonNull<u8> {
        self.primary.pointer()
    }

    /// Returns `true` if the pointer is within memory owned by the allocator
    pub fn owns(&self, ptr: *const u8) -> bool {
        if self.primary.owns(ptr) {
            true
        } else if let Some(ref backup) = self.backup {
            backup.owns(ptr)
        } else {
            false
        }
    }

    pub unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, alloc::AllocErr> {
        debug_log!("SizedAllocator: allocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if let memory@Ok(_) = self.primary.alloc(layout) {
            memory
        } else {
            let backup = self.backup.as_mut().ok_or(alloc::AllocErr)?;
            backup.alloc(layout)
        }
    }

    pub unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) -> DeallocResponse {
        debug_log!("SizedAllocator: deallocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.dealloc(ptr, layout);
            if self.primary.is_empty() {
                DeallocResponse::Collapse
            } else {
                DeallocResponse::Nothing
            }
        } else if let Some(backup) = self.backup.take() {
            debug_log!("    (Primary does not own it)\n\0");
            match backup.dealloc(ptr, layout) {
                DeallocResponse::Collapse => {
                    backup.primary.debug_assert_empty();
                    self.backup = backup.backup.take();
                    DeallocResponse::FreeAllocator(backup)
                },
                x => {
                    self.backup = Some(backup);
                    x
                },
            }
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to dealloc, there must be a backup")
        }
    }

    pub unsafe fn shrink_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) {
        debug_log!("SizedAllocator: attempting to shrink size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr.as_ptr());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.shrink_in_place(ptr, layout, new_size)
        } else if let Some(ref mut backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.shrink_in_place(ptr, layout, new_size)
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to shrink, there must be a backup")
        }
    }

    pub unsafe fn grow_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        debug_log!("SizedAllocator: attempting to grow size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr.as_ptr());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.grow_in_place(ptr, layout, new_size)
        } else if let Some(ref mut backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.grow_in_place(ptr, layout, new_size)
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to grow, there must be a backup")
        }
    }
}
