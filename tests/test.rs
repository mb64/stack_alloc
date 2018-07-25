#![feature(const_fn)]

extern crate stack_alloc;

use stack_alloc::{Allocator, TestMemorySource};

#[global_allocator]
static GLOBAL: Allocator<TestMemorySource> = Allocator::new();

#[test]
fn vecs() {
    let mut my_vec = vec![1, 2, 3];
    for i in 0..7 {
        my_vec.push(i);
    }
    assert_eq!(my_vec, [1, 2, 3, 0, 1, 2, 3, 4, 5, 6]);
}

#[test]
fn string() {
    let mut my_string = "Hello! This is a String.".to_owned();
    my_string += "\nYes it is.";
    println!("{}", my_string);
}
