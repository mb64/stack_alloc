//! This module implements the method for managing stacks of a given size.
//!
//! TODO: how to create a SizedAllocator?

use core::alloc::{self, Alloc, Layout};
use core::cell;
use core::ptr::NonNull;

use bitmapped_stack::{BitmappedStack, STACK_SIZE};
use memory_source::{self, MemorySource};

/*/// A `SizedAllocator` gets new memory from a `Factory`.
#[derive(Debug)]
pub enum Factory {
    /// The factory is another, bigger, `SizedAllocator`
    SizedAlloc(&'static SizedAllocator),
    /// The factory is a MemorySource whose allocation function is this one
    MemorySource(unsafe fn() -> Option<ptr::NonNull<u8>>),
    /// There is no factory
    None,
}
*/

/// The `SizedAllocator` type is the type that implements `GlobalAlloc`
///
/// It uses internal mutability to make it work
///
/// A `SizedAllocator` is a/*n automatically-expanding*/ linked list of stacks whose chunk size is the
/// same.
#[derive(Debug)]
pub struct SizedAllocator {
    cell: cell::RefCell<BackupAllocator>,
}

impl SizedAllocator {
    /// Allocates memory from the factory and creates a new `SizedAllocator` using the given chunk
    /// size.
    ///
    /// It panics if it can't allocate memory from the factory
    pub unsafe fn from_sized_alloc_factory(chunk_size: usize, factory: &'static SizedAllocator, backup: Option<&'static SizedAllocator>) -> Option<Self> {
        let memory = {
            let size = STACK_SIZE * chunk_size;
            let align = chunk_size;
            debug_assert_eq!(size, factory.chunk_size());
            factory.alloc(Layout::from_size_align(size, align).unwrap())?
        };

        Some(Self::from_backup(
            BackupAllocator {
                primary: BitmappedStack::new(memory, chunk_size),
                backup: backup,
                //factory: Factory::SizedAlloc(factory),
            }))
    }

    /// Makes a new `SizedAllocator` that uses the `MemorySource` as its factory
    pub unsafe fn from_memory_source<T: MemorySource>(chunk_size: usize, backup: Option<&'static SizedAllocator>) -> Option<Self> {
        debug_assert_eq!(chunk_size * 64, memory_source::BLOCK_SIZE);
        let memory = T::get_block()?;
        Some(Self::from_backup(
            BackupAllocator {
                primary: BitmappedStack::new(memory, chunk_size),
                backup: backup,
                //factory: Factory::MemorySource(func),
            }))
    }

    /// Makes a new `SizedAllocator` that uses the given stack
    fn from_stack(stack: BitmappedStack) -> Self {
        Self::from_backup(
            BackupAllocator {
                primary: stack,
                backup: None,
                //factory: Factory::None,
            })
    }

    /// Makes a `SizedAllocator` from a given `BackupAllocator`
    fn from_backup(backup: BackupAllocator) -> Self {
        SizedAllocator {
            cell: cell::RefCell::new(backup),
        }
    }

    /// Returns the chunk size of this allocator
    pub fn chunk_size(&self) -> usize {
        self.cell.borrow().chunk_size()
    }

    /// Returns whether or not the pointer is within memory owned by the allocator
    pub fn owns(&self, ptr: *const u8) -> bool {
        self.cell.borrow().owns(ptr)
    }

    /// Tries to shrink the given memory.
    ///
    /// Performs the same operation as `core::alloc::Alloc::shrink_in_place`
    pub unsafe fn shrink_in_place(&self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        self.cell.borrow_mut().shrink_in_place(ptr, layout, new_size)
    }

    /// Tries to grow the given memory.
    ///
    /// Performs the same operation as `core::alloc::Alloc::grow_in_place`
    pub unsafe fn grow_in_place(&self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        self.cell.borrow_mut().grow_in_place(ptr, layout, new_size)
    }

    /// Tries to allocate memory of the given layout.
    pub unsafe fn alloc(&self, layout: Layout) -> Option<NonNull<u8>> {
        debug_log!("SizedAllocator: allocing size %zu\n\0", layout.size());
        self.cell.borrow_mut().alloc(layout).ok()
    }

    /// Deallocates the memory
    pub unsafe fn dealloc(&self, ptr: NonNull<u8>, layout: Layout) {
        debug_log!("SizedAllocator: deallocing size %zu\n\0", layout.size());
        self.cell.borrow_mut().dealloc(ptr, layout);
    }

    /*pub unsafe fn realloc(&self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Option<NonNull<u8>> {
        debug_log!("SizedAllocator: reallocing size %zu pointer %#zx\n\0", layout.size(), ptr);
        self.cell.borrow_mut().realloc(ptr, layout, new_size).ok()
    }*/
}

/// An allocator with a potential backup for allocation failures.
/// It allocates new `SizedAllocator`s as necessary.
#[derive(Debug)]
struct BackupAllocator {
    primary: BitmappedStack,
    /// The backup allocator should have the same size chunk as the primary allocator
    backup: Option<&'static SizedAllocator>,
    // /// The factory's chunk size will be enough to fit an entire block of
    // factory: Factory,
}

impl BackupAllocator {
    /// Returns the smallest size allocation possible
    fn chunk_size(&self) -> usize {
        let size = self.primary.chunk_size();

        /*#[cfg(debug_asserts)]
        {
            if let Some(backup) = self.backup {
                debug_assert_eq!(size, backup.chunk_size());
            }
        }*/

        size
    }

    /*/// Returns the backup allocator, making a new one with memory from the factory if necessary
    fn get_backup(&mut self) -> Option<&'static SizedAllocator> {
        debug_log!("Using backup allocator\n\0");
        if let alloc@Some(_) = self.backup {
            alloc
        } else {
            match self.factory {
                Factory::SizedAlloc(sized_alloc) => {
                    let new_allocator = SizedAllocator::from_sized_alloc_factory(sized_alloc, self.chunk_size());
                    self.backup = Some(metadata_allocator::store_metadata(new_allocator));
                    self.backup
                },
                Factory::MemorySource(func) => {
                    let new_allocator = SizedAllocator::from_memory_source_func(func, self.chunk_size())?;
                    self.backup = Some(metadata_allocator::store_metadata(new_allocator));
                    self.backup
                },
                Factory::None => None,
            }
        }
    }*/

    /// Returns `true` if the pointer is within memory owned by the allocator
    fn owns(&self, ptr: *const u8) -> bool {
        if self.primary.owns(ptr) {
            true
        } else if let Some(backup) = self.backup {
            backup.owns(ptr)
        } else {
            false
        }
    }
}

unsafe impl Alloc for BackupAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<NonNull<u8>, alloc::AllocErr> {
        debug_log!("BackupAllocator: allocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if let memory@Ok(_) = self.primary.alloc(layout) {
            memory
        } else {
            let backup = self.backup.ok_or(alloc::AllocErr)?;
            backup.alloc(layout).ok_or(alloc::AllocErr)
        }
    }

    unsafe fn dealloc(&mut self, ptr: NonNull<u8>, layout: Layout) {
        debug_log!("BackupAllocator: deallocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.dealloc(ptr, layout);
        } else if let Some(backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.dealloc(ptr, layout);
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to dealloc, there must be a backup")
        }
    }

    unsafe fn shrink_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        debug_log!("BackupAllocator: attempting to shrink size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr.as_ptr());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.shrink_in_place(ptr, layout, new_size)
        } else if let Some(backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.shrink_in_place(ptr, layout, new_size)
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to shrink, there must be a backup")
        }
    }

    unsafe fn grow_in_place(&mut self, ptr: NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        debug_log!("BackupAllocator: attempting to grow size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr.as_ptr());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.grow_in_place(ptr, layout, new_size)
        } else if let Some(backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.grow_in_place(ptr, layout, new_size)
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to grow, there must be a backup")
        }
    }
}

