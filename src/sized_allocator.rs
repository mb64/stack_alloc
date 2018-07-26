//! This module implements the method for managing linked lists of stacks of a consistent size.

use core::alloc::{self, Layout};
use core::cmp;
use core::ptr::NonNull;

use bitmapped_stack::BitmappedStack;
use metadata_box::MetadataBox;

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
    FreeAllocator(MetadataBox<SizedAllocator>),
}

/// A `SizedAllocator` is a linked list of stacks whose chunk size is the same.
#[derive(Debug)]
pub struct SizedAllocator {
    primary: BitmappedStack,
    /// The backup allocator should have the same size chunk as the primary allocator
    backup: Option<MetadataBox<SizedAllocator>>,
    /// The largest contiguous group of memory left in both the primary and backup
    largest_space_left: usize,
}

impl SizedAllocator {
    /// Create a new `SizedAllocator` from the given chunk of memory
    ///
    /// # Safety
    ///
    /// The caller must ensure that:
    ///  * The chunk size is a power of 2
    ///  * The memory is a valid pointer with alignment `chunk_size` and size `STACK_SIZE *
    ///  chunk_size`
    pub unsafe fn from_memory_chunk(chunk_size: usize, memory: NonNull<u8>, backup: Option<MetadataBox<SizedAllocator>>) -> Self {
        SizedAllocator {
            primary: BitmappedStack::new(memory, chunk_size),
            backup: backup,
            largest_space_left: 64,
        }
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

    /// Returns `true` if it owns the memory
    pub fn owns(&self, ptr: NonNull<u8>) -> bool {
        if self.primary.owns(ptr.as_ptr()) {
            true
        } else if let Some(backup) = &self.backup {
            backup.owns(ptr)
        } else {
            false
        }
    }

    fn set_largest_space_left(&mut self) {
        let backup_space_left = match &self.backup {
            Some(backup) => backup.largest_space_left,
            None => 0,
        };
        self.largest_space_left = cmp::max(self.primary.chunks_left(), backup_space_left);
    }

    pub unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, alloc::AllocErr> {
        debug_log!("SizedAllocator: allocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if layout.size() > self.chunk_size() * self.largest_space_left {
            debug_log!("  (short-circuiting the list because it's too big)\n\0");
            return Err(alloc::AllocErr);
        }
        if let memory@Ok(_) = self.primary.alloc(layout) {
            self.set_largest_space_left();
            memory
        } else {
            let backup = self.backup.as_mut().ok_or(alloc::AllocErr)?;
            let res = backup.alloc(layout);
            self.set_largest_space_left();
            res
        }
    }

    pub unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) -> DeallocResponse {
        debug_log!("SizedAllocator: deallocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.dealloc(ptr, layout);
            self.set_largest_space_left();
            if self.primary.is_empty() {
                DeallocResponse::Collapse
            } else {
                DeallocResponse::Nothing
            }
        } else if let Some(mut backup) = self.backup.take() {
            debug_log!("    (Primary does not own it)\n\0");
            match backup.dealloc(ptr, layout) {
                DeallocResponse::Collapse => {
                    backup.primary.debug_assert_empty();
                    self.backup = backup.backup.take();
                    self.set_largest_space_left();
                    DeallocResponse::FreeAllocator(backup)
                },
                x => {
                    self.backup = Some(backup);
                    self.set_largest_space_left();
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
            self.primary.shrink_in_place(ptr, layout, new_size);
            self.set_largest_space_left();
        } else if let Some(ref mut backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.shrink_in_place(ptr, layout, new_size);
            self.set_largest_space_left();
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to shrink, there must be a backup")
        }
    }

    pub unsafe fn grow_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        debug_log!("SizedAllocator: attempting to grow size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr.as_ptr());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            match self.primary.grow_in_place(ptr, layout, new_size) {
                Ok(()) => {
                    self.set_largest_space_left();
                    Ok(())
                },
                err => err,
            }
        } else if let Some(ref mut backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            match backup.grow_in_place(ptr, layout, new_size) {
                Ok(()) => {
                    self.set_largest_space_left();
                    Ok(())
                },
                err => err,
            }
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to grow, there must be a backup")
        }
    }
}
