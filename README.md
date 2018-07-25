# `stack_alloc` - a memory allocator

## How to use it

You need a couple things to use it:
 * A nightly compiler
 * A place to get memory

Add to your `Cargo.toml`:
```toml
[dependencies]
stack_alloc = {git = "https://github.com/mb64/stack_alloc.git"}
```

Now you can set it as the default allocator:
```rust
#![feature(const_fn)]

extern crate stack_alloc;

use stack_alloc::{Allocator, MemorySource};

struct MyMemorySource;
unsafe impl MemorySource for MyMemorySource {
    unsafe fn get_block() -> Option<std::ptr::NonNull<u8>> {
        // Get a 256 KiB chunk of memory somehow...
        unimplemented!()
    }
}

#[global_allocator]
struct GLOBAL: Allocator<MyMemorySource> = Allocator::new();
```

## Features

`stack_alloc` has some great (and some slightly-less-great) features:

### It's (minimally) thread-safe.

It uses a single top-level allocator lock so that only one thread can allocate memory at a time.

### It's pretty slow.

The only kind of a benchmark I've done is pretty simple.

 * I downloaded two copies of Ripgrep [from git](https://github.com/BurntSushi/ripgrep), and edited one to use this allocator with a simple test
   memory source
 * I built each with a simple `cargo build --release`
 * Then, I ran `time target/release/rg -Hn -j1 -e 'ripgrep' >/dev/null` several times in each directory and recorded the fastest time for each.

The numbers were:
 * Ripgrep with `stack_alloc`: 35 ms
 * Ripgrep with Jemalloc: 32 ms

So it's not intolerably slow, but it's still pretty slow.

### It's flexible in where it gets its memory

Whatever way you have to get memory, it's probably possible to use it.  Just implement `MemorySource`, and you're all set!

## Overall design

There are (roughly) 4 layers to the design:

 * At the simplest layer, there's a bunch of stacks, of different sizes.  However, because you can't deallocate with a stack, each stack also has a
   64-bit bitmap of its contents so it knows when it can lower its stack pointer.  (This is in the file `src/bitmapped_stack.rs`.)
 * Next, there are linked lists of stacks.  Each linked list has stacks of a consistent size.  If the first stack can't allocate a thing, it'll keep
   going down the list to try to find one that can allocate it.  (This is in the file `src/sized_allocator.rs`.)
 * Based on the size of the allocation, it will be given to different lists, whose stacks are of different sizes.  If a linked list of stacks runs
   out of room, another stack is added to the head, with memory allocated from the next-size-up linked list.  (This is in the file
   `src/factory_chain.rs`.)
 * Finally, there's a lock at the top for minimal thread safety.

