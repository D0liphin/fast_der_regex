#![feature(
    vec_push_within_capacity,
    allocator_api,
    alloc_layout_extra,
    slice_ptr_get
)]

pub mod regex_v2;
pub mod vec_alloc;
use std::hint::black_box;

use vec_alloc::*;
pub mod regex;
use regex::*;

fn main() {
    let mut regex = Regex::new();
    let alloc = unsafe { regex.alloc_mut() };
    let tree = alloc.alloc(Re::Char('a')).unwrap().into();
    let tree = alloc.alloc(Re::Star(tree)).unwrap().into();
    let tree = alloc.alloc(Re::Star(tree)).unwrap().into();
    let tree = Re::Seq(tree, alloc.alloc(Re::Char('b')).unwrap().into());
    drop(alloc);
    unsafe {
        *regex.tree_mut() = tree;
    }

    dbg!(&regex);
    dbg!(regex.der('a').simp().to_owned().der('b').simp());

    let mut s = String::from("a".repeat(10000));
    s.push('b');
    let earlier = std::time::Instant::now();
    let mut result = 0;
    for _ in 0..1000 {
        if black_box(regex.is_match(black_box(&s))) {
            result += 1;
        }
    }
    println!("{}", result);
    dbg!(std::time::Instant::now().duration_since(earlier));
}
