#![feature(
    vec_push_within_capacity,
    allocator_api,
    alloc_layout_extra,
    slice_ptr_get
)]

pub mod vec_alloc;
use std::hint::black_box;

pub mod regex;
use regex::build_plan::ImplicitRe;
use regex::Regex;

fn main() {
    let regex = Regex::from(&'a'.star().star().seq('b'));
    println!("regex.mem_size() = {:?}", regex.alloc());
    dbg!(&regex);
    // dbg!(unsafe { regex.alloc_mut() });
    let der = regex.der('a');
    println!("der = {:?}", der);   
    println!("der.alloc() = {:?}", der.alloc());
    println!("der.clone().alloc() = {:?}", der.clone().alloc());

    // let mut s = "a".repeat(100);
    // s.push('b');
    // let earlier = std::time::Instant::now();
    // let mut result = 0;
    // for _ in 0..1000 {
    //     if black_box(regex.is_match(black_box(&s))) {
    //         result += 1;
    //     }
    // }
    // println!("{}", result);
    // dbg!(std::time::Instant::now().duration_since(earlier));
}
