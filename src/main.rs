//! A simple-ish allocator
//!
//! It's not done

//#![no_std]
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
mod sized_allocator;
mod metadata_allocator;
pub mod global;
mod factory_chain;
pub mod memory_source;
pub mod global_allocator;

pub use memory_source::MemorySource;
pub use global_allocator::Allocator;

#[global_allocator]
static GLOBAL: Allocator<global::MyGreatMemorySource> = Allocator::new();

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
