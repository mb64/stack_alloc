//! Where the allocator gets its memory chunks
//!
//! ```rust
//! use core::ptr::NonNull;
//! use stack_alloc::memory_source::{MemorySource, Fallback, ...}
//!
//! struct MyUnreliableMemorySource;
//!
//! impl MemorySource for MyUnreliableMemorySource {
//!     fn get_block() -> Option<NonNull<u8>> {
//!         // Try to get memory...
//!         None
//!     }
//! }
//!
//! type MyReliableMemorySource = Fallback<MyUnreliableMemorySource, TODO>;
//! ```

use core::marker::PhantomData;
use core::ptr::NonNull;

/// The size, in bytes, of a returned block
///
/// A returned block needs to have a size of 256 KiB and an alignment of 4 KiB.
pub const BLOCK_SIZE: usize = 262144;

/// The alignment, in bytes, of a returned block
///
/// A returned block needs to have a size of 256 KiB and an alignment of 4 KiB.
pub const BLOCK_ALIGN: usize = 4096;

/// The `MemorySource` trait is used to allow for different backends for obtaining memory.
///
/// For example, in web assembly, the way to get memory is different from on Linux, and in a
/// bare-metal situation you'd have to make your own stack or something.
pub unsafe trait MemorySource {
    /// Potentially returns a block of memory.
    ///
    /// This memory needs to fulfill layout requirements:
    ///  * It should be 256 KiB (262144 bytes) large
    ///  * It should be aligned to 4 KiB (4096 bytes)
    ///
    /// If it returns `Some(thing)`, then ownership of the block of memory pointed to by `thing` is
    /// transferred to the caller.
    unsafe fn get_block() -> Option<NonNull<u8>>;
}

/// A memory source that is never successful in returning memory.
///
/// If you use it, you won't have any memory.  Even simple things that need allocation,
/// `println!()` for example, won't work.  (`println!` allocates an output buffer.)
///
/// `NoMemory` is the unit source with respect to `Fallback`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct NoMemory;

unsafe impl MemorySource for NoMemory {
    unsafe fn get_block() -> Option<NonNull<u8>> {
        None
    }
}

/// `Fallback<T, U>` first tries to get memory from `T`, but gets it from `U` if that is
/// unsuccessful.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash, Debug)]
pub struct Fallback<T, U> {
    _data: PhantomData<(T, U)>,
}

unsafe impl<T, U> MemorySource for Fallback<T, U>
    where T: MemorySource,
          U: MemorySource
{
    unsafe fn get_block() -> Option<NonNull<u8>> {
        T::get_block().or_else(|| U::get_block())
    }
}
