#![feature(
    vec_push_within_capacity,
    allocator_api,
    alloc_layout_extra,
    slice_ptr_get
)]

pub mod vec_alloc;
use std::hint::black_box;
use std::str::FromStr;

pub mod regex;
use regex::build_plan::ImplicitRe;
use regex::Regex;

fn main() {
    let regex = Regex::from(&'a'.star().seq('b'));
    let mut s = "a".repeat(1000);
    s.push_str("bbbb");
    let earlier = std::time::Instant::now();
    let mut result = 0;
    for _ in 0..1000 {
        if black_box(regex.is_match(black_box(&s))) {
            result += 1;
        }
    }
    println!("{}", result);
    dbg!(std::time::Instant::now().duration_since(earlier));

    let regex = rust_regex::Regex::from_str("a*b").unwrap();
    let mut s = "a".repeat(1000);
    s.push_str("bbbb");
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
