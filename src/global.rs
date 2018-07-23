//! A simple global allocator for testing

use alloc::alloc::{Layout, GlobalAlloc};
use core::ptr;

use bitmapped_stack::BitmappedStack;
use sized_allocator::SizedAllocator;
use metadata_allocator;

/// A simple allocator for testing purposes
#[derive(Clone, Copy, Debug)]
pub struct SimpleTestAlloc;

static mut FACTORY_MEMORY: [[[u8; 64]; 64]; 64] = [[[0; 64]; 64]; 64];

static mut FACTORY: Option<&'static SizedAllocator> = None;

fn get_factory() -> &'static SizedAllocator {
    unsafe {
        if let Some(factory) = FACTORY {
            factory
        } else {
            debug_log!("Making the factory\n\0");
            let factory = {
                let ptr = {
                    let ptr: *mut _ = &mut FACTORY_MEMORY;
                    ptr::NonNull::new_unchecked(ptr as *mut u8)
                };
                let stack = BitmappedStack::new(ptr, 64*64);
                let sized = SizedAllocator::from_stack(stack);
                metadata_allocator::store_metadata(sized)
            };
            FACTORY = Some(factory);
            factory
        }
    }
}

static mut THE_ALLOC: Option<&'static SizedAllocator> = None;
fn get_alloc() -> &'static SizedAllocator {
    unsafe {
        if let Some(alloc) = THE_ALLOC {
            alloc
        } else {
            debug_log!("Making the allocator\n\0");
            let alloc = {
                let sized = SizedAllocator::from_factory(get_factory(), 64);
                metadata_allocator::store_metadata(sized)
            };
            THE_ALLOC = Some(alloc);
            debug_log!("Done making the allocator\n\0");
            alloc
        }
    }
}

unsafe impl GlobalAlloc for SimpleTestAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        debug_log!("SimpleTestAlloc: allocing size %zu align %zu\n\0", layout.size(), layout.align());
        let ptr = get_alloc().alloc(layout);
        debug_log!("SimpleTestAlloc: done allocing size %zu align %zu; the pointer is %#zx\n\n\0", layout.size(), layout.align(), ptr);
        ptr
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        debug_log!("SimpleTestAlloc: deallocing size %zu align %zu pointer %#zx\n\0", layout.size(), layout.align(), ptr);
        get_alloc().dealloc(ptr, layout);
        debug_log!("SimpleTestAlloc: done deallocing size %zu align %zu pointer %#zx\n\n\0", layout.size(), layout.align(), ptr);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        debug_log!("SimpleTestAlloc: reallocing size %zu to %zu align %zu pointer %#zx\n\0", layout.size(), new_size, layout.align(), ptr);
        let ptr = get_alloc().realloc(ptr, layout, new_size);
        debug_log!("SimpleTestAlloc: done reallocing size %zu align %zu; the new pointer is %#zx\n\n\0", layout.size(), layout.align(), ptr);
        ptr
    }
}

/*static mut THE_ALLOCATOR: BitmappedStack = {
    let ptr = unsafe {
        let ptr: *mut _ = &mut THE_MEMORY;
        NonNull::new_unchecked(ptr as *mut u8)
    };
    BitmappedStack::new(ptr, 64)
};

fn to_raw<E>(x: Result<NonNull<u8>, E>) -> *mut u8 {
    if let Ok(nonnull) = x {
        nonnull.as_ptr()
    } else {
        null_mut()
    }
}

unsafe impl GlobalAlloc for SimpleTestAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        //let mut allocator = THE_ALLOCATOR.lock().unwrap();
        let allocator = &mut THE_ALLOCATOR;
        to_raw(allocator.alloc(layout))
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        //let mut allocator = THE_ALLOCATOR.lock().unwrap();
        let allocator = &mut THE_ALLOCATOR;
        allocator.dealloc(ptr::NonNull::new(ptr).unwrap(), layout);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let allocator = &mut THE_ALLOCATOR;
        let ptr = NonNull::new(ptr).unwrap();
        to_raw(allocator.realloc(ptr, layout, new_size))
    }
}
*/
