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

#![no_std]
#![feature(nll)]
#![feature(alloc)]
#![feature(allocator_api)]
#![feature(ptr_offset_from)]
#![feature(const_fn, const_let)]
#![feature(cell_update)]
#![warn(
    missing_docs,
    missing_debug_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unused_import_braces,
    unused_qualifications
)]

extern crate alloc;

#[cfg(any(feature = "debug_logs", feature = "test_memory_source"))]
extern crate libc;

#[macro_use]
mod macros;
mod bitmapped_stack;
mod factory_chain;
pub mod global_allocator;
pub mod memory_source;
mod metadata_box;
mod sized_allocator;

#[cfg(feature = "test_memory_source")]
mod test_memory_source;
#[cfg(feature = "test_memory_source")]
pub use test_memory_source::TestMemorySource;

pub use global_allocator::Allocator;
pub use memory_source::MemorySource;
