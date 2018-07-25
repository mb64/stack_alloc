//! A simple-ish allocator
//!
//! # How to use it
//!
//! ## Memory sources
//!
//! This library is flexible in how/where to get the memory.  In different environments and
//! situations, you might want to make a kernel call, do a WebAssembly thing, or whatever it is
//! that Windows does.
//!
//! Make a memory source:
//! ```no_run
//! extern crate stack_alloc;
//! use stack_alloc::MemorySource;
//!
//! struct MyAmazingMemorySource;
//!
//! unsafe impl MemorySource for MyAmazingMemorySource {
//!     unsafe fn get_block() -> Option<std::ptr::NonNull<u8>> {
//!         // Get a 4096-aligned 256 KiB chunk of memory ...
//!         unimplemented!()
//!     }
//! }
//! ```
//!
//! ## Setting the global allocator
//!
//! Now, you need to tell the compiler that you want to use this as your allocator:
//!
//! ```no_run
//! #![feature(const_fn)]
//!
//! extern crate stack_alloc;
//! use stack_alloc::Allocator;
//!
//! struct MyAmazingMemorySource;
//! unsafe impl stack_alloc::MemorySource for MyAmazingMemorySource {
//!    unsafe fn get_block() -> Option<std::ptr::NonNull<u8>> { unimplemented!() }
//! }
//!
//! #[global_allocator]
//! static GLOBAL: Allocator<MyAmazingMemorySource> = Allocator::new();
//! ```
//!
//! ## Allocating things
//!
//! Now you can allocate all you want: all the memory used in `Box`, `Vec`, `String`, etc. will be
//! obtained from `MyAmazingMemorySource` and then managed by the library.

//#![no_std]
#![feature(nll)]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(ptr_offset_from)]
#![feature(const_fn, const_let)]
#![feature(cell_update)]

#![warn(missing_docs,
        missing_debug_implementations,
        trivial_casts, trivial_numeric_casts,
        unused_import_braces, unused_qualifications)]

extern crate core;
extern crate alloc;
extern crate libc;

#[macro_use]
mod macros;
mod bitmapped_stack;
mod metadata_box;
mod sized_allocator;
//mod metadata_allocator;
mod factory_chain;
pub mod memory_source;
pub mod global_allocator;

pub mod test_memory_source;

pub use memory_source::MemorySource;
pub use global_allocator::Allocator;

/*#[global_allocator]
static GLOBAL: Allocator<test_memory_source::MyGreatMemorySource> = Allocator::new();

fn main() {
    let _v0: Vec<u8> = Vec::with_capacity(4096);
    let _v1: Vec<u8> = Vec::with_capacity(9);
    println!("The layout of a SizedAllocator is {:#?}", core::alloc::Layout::new::<sized_allocator::SizedAllocator>());
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_test() {
        string();
        vec();

        use bitmapped_stack::BitmappedStack;
        use alloc::alloc::{Alloc, Layout};
        use core::ptr;
        unsafe {
            static mut MEMORY: [u64; 64] = [0; 64];
            let mut allocator = BitmappedStack::new(
                ptr::NonNull::new(&mut MEMORY).unwrap().cast(),
                8, // Bytes per chunk
                );
            println!("allocator: {:#?}", allocator);

            {
                let the_layout = Layout::from_size_align(28, 4).unwrap();
                let the_ptr = allocator.alloc(the_layout).unwrap().cast::<[i32; 7]>();
                allocator.debug_assert_nonempty();
                the_ptr.as_ptr().write([3, 4, 6, 7, 1, 2, -3]);
                for i in 0..7 {
                    println!("{}", the_ptr.cast::<i32>().as_ptr().offset(i).read());
                }
                println!("allocator: {:#?}", allocator);
                allocator.dealloc(the_ptr.cast(), the_layout);
                allocator.debug_assert_empty();
                println!("allocator: {:#?}", allocator);
            }

            let too_big = Layout::from_size_align(1024, 8).unwrap();
            assert!(allocator.alloc(too_big).is_err());

            {
                allocator.debug_assert_empty();
                let not_too_big = Layout::from_size_align(496, 8).unwrap();
                let big_ptr = allocator.alloc(not_too_big).unwrap();
                println!("allocator: {:#?}", allocator);
                allocator.dealloc(big_ptr, not_too_big);
            }

            {
                let weirdly_aligned = Layout::from_size_align(13, 128).unwrap();
                let yep = allocator.alloc(weirdly_aligned).unwrap();
                println!("allocator after weird alignment: {:#?}", allocator);
                allocator.debug_assert_nonempty();
                allocator.dealloc(yep, weirdly_aligned);
            }

            println!("allocator: {:#?}", allocator);
            allocator.debug_assert_empty();
        }
    }

    fn string() {
        let my_string = "Hello!".to_owned();
        assert_eq!(&my_string, "Hello!");
    }

    fn vec() {
        let mut my_vec = vec![1, 2, 4, 5];
        assert_eq!(&my_vec, &[1, 2, 4, 5]);
        my_vec.push(3);
        my_vec.push(-4);
        assert_eq!(&my_vec, &[1, 2, 4, 5, 3, -4]);
    }
}
*/
