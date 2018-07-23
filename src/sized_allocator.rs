//! This module implements the method for managing stacks of a given size.
//! 
//! TODO: how to create a SizedAllocator?

use core::alloc::{self, Alloc, GlobalAlloc, Layout};
use core::cell;
use core::ptr;

use bitmapped_stack::{BitmappedStack, STACK_SIZE};
use metadata_allocator;

/// The `SizedAllocator` type is the type that implements `GlobalAlloc`
///
/// It uses internal mutability to make it work
///
/// A `SizedAllocator` is an automatically-expanding linked list of stacks whose chunk size is the
/// same.
#[derive(Debug)]
pub struct SizedAllocator {
    cell: cell::RefCell<BackupAllocator>,
}

impl SizedAllocator {
    /// Allocates memory from the factory and creates a new `SizedAllocator` using that factory and
    /// the given chunk size
    /// 
    /// It panics if it can't allocate memory from the factory
    pub fn from_factory(factory: &'static SizedAllocator, chunk_size: usize) -> Self {
        let memory = {
            let size = STACK_SIZE * chunk_size;
            let align = chunk_size;
            debug_assert_eq!(size, factory.chunk_size());
            unsafe {
                let raw_ptr = factory.alloc(Layout::from_size_align(size, align).unwrap());
                ptr::NonNull::new(raw_ptr).expect("Couldn't allocate memory from the factory")
            }
        };

        Self::from_backup(
            BackupAllocator {
                primary: BitmappedStack::new(memory, chunk_size),
                backup: None,
                factory: Some(factory),
            })
    }

    /// Makes a new `SizedAllocator` that uses the given stack
    pub fn from_stack(stack: BitmappedStack) -> Self {
        Self::from_backup(
            BackupAllocator {
                primary: stack,
                backup: None,
                factory: None,
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

    /// Tries to shrink the given memory.
    ///
    /// Performs the same operation as `core::alloc::Alloc::shrink_in_place`
    unsafe fn shrink_in_place(&self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        self.cell.borrow_mut().shrink_in_place(ptr, layout, new_size)
    }

    /// Tries to grow the given memory.
    ///
    /// Performs the same operation as `core::alloc::Alloc::grow_in_place`
    unsafe fn grow_in_place(&self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
        self.cell.borrow_mut().grow_in_place(ptr, layout, new_size)
    }
}

unsafe impl GlobalAlloc for SizedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_log!("SizedAllocator: allocing size %zu\n\0", layout.size());
        if let Ok(nonnull) = self.cell.borrow_mut().alloc(layout) {
            nonnull.as_ptr()
        } else {
            ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_log!("SizedAllocator: deallocing size %zu\n\0", layout.size());
        let nonnull = ptr::NonNull::new(ptr).expect("The given pointer to dealloc was null");
        self.cell.borrow_mut().dealloc(nonnull, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        debug_log!("SizedAllocator: reallocing size %zu pointer %#zx\n\0", layout.size(), ptr);
        let nonnull = ptr::NonNull::new(ptr).expect("The given pointer to realloc was null");
        if let Ok(res) = self.cell.borrow_mut().realloc(nonnull, layout, new_size) {
            res.as_ptr()
        } else {
            ptr::null_mut()
        }
    }
}

/// An allocator with a potential backup for allocation failures.
/// It allocates new `SizedAllocator`s as necessary.
#[derive(Debug)]
struct BackupAllocator {
    primary: BitmappedStack,
    /// The backup allocator should have the same size chunk as the primary allocator
    backup: Option<&'static SizedAllocator>,
    /// The factory's chunk size will be enough to fit an entire block of 
    factory: Option<&'static SizedAllocator>,
}

impl BackupAllocator {
    /// Returns the smallest size allocation possible
    fn chunk_size(&self) -> usize {
        let size = self.primary.chunk_size();

        #[cfg(debug_asserts)]
        {
            if let Some(backup) = self.backup {
                debug_assert_eq!(size, backup.chunk_size());
            }
        }

        size
    }

    /// Returns the backup allocator, making a new one with memory from the factory if necessary
    fn get_backup(&mut self) -> Option<&'static SizedAllocator> {
        debug_log!("Using backup allocator\n\0");
        if let alloc@Some(_) = self.backup {
            alloc
        } else if let Some(factory) = self.factory {
            let new_allocator = SizedAllocator::from_factory(factory, self.chunk_size());
            self.backup = Some(metadata_allocator::store_metadata(new_allocator));
            self.backup
        } else {
            None
        }
    }
}

unsafe impl Alloc for BackupAllocator {
    unsafe fn alloc(&mut self, layout: Layout) -> Result<ptr::NonNull<u8>, alloc::AllocErr> {
        debug_log!("BackupAllocator: allocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if let memory@Ok(_) = self.primary.alloc(layout) {
            memory
        } else {
            let backup = self.get_backup().ok_or(alloc::AllocErr)?;
            ptr::NonNull::new(backup.alloc(layout)).ok_or(alloc::AllocErr)
        }
    }

    unsafe fn dealloc(&mut self, ptr: ptr::NonNull<u8>, layout: Layout) {
        debug_log!("BackupAllocator: deallocing size %zu, align %zu\n\0", layout.size(), layout.align());
        if self.primary.owns(ptr.as_ptr()) {
            debug_log!("    (Primary owns it)\n\0");
            self.primary.dealloc(ptr, layout);
        } else if let Some(backup) = self.backup {
            debug_log!("    (Primary does not own it)\n\0");
            backup.dealloc(ptr.as_ptr(), layout);
        } else {
            debug_log!("    (Primary does not own it, and there is no backup)\n\0");
            unreachable!("If the primary doesn't own the memory to dealloc, there must be a backup")
        }
    }

    unsafe fn shrink_in_place(&mut self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
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

    unsafe fn grow_in_place(&mut self, ptr: ptr::NonNull<u8>, layout: Layout, new_size: usize) -> Result<(), alloc::CannotReallocInPlace> {
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

