use std::{fmt, marker::PhantomData};

use crate::vec_alloc::VecAlloc;

pub mod const_ptr;
pub use const_ptr::*;
pub mod build_plan;

#[derive(Clone, Copy)]
pub enum Re {
    Zero,
    One,
    Char(char),
    Alt(Const<Re>, Const<Re>),
    Seq(Const<Re>, Const<Re>),
    Star(Const<Re>),
}

impl Re {
    // TODO: make #[tailcall]
    pub fn nullable(&self) -> bool {
        match &self {
            Re::Zero => false,
            Re::One => true,
            Re::Char(_) => false,
            Re::Alt(r1, r2) => unsafe { r1.as_ref().nullable() || r2.as_ref().nullable() },
            Re::Seq(r1, r2) => unsafe { r1.as_ref().nullable() && r2.as_ref().nullable() },
            Re::Star(_) => true,
        }
    }

    unsafe fn const_eq(lhs: Const<Re>, rhs: Const<Re>) -> bool {
        lhs.eq(rhs, |a, b| Re::eq(a, b))
    }

    unsafe fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Zero, Self::Zero) => true,
            (Self::One, Self::One) => true,
            (Self::Char(c), Self::Char(d)) => c == d,
            (Self::Alt(l1, l2), Self::Alt(r1, r2)) => unsafe {
                Self::const_eq(*l1, *r1) && Self::const_eq(*l2, *r2)
            },
            (Self::Seq(l1, l2), Self::Seq(r1, r2)) => unsafe {
                Self::const_eq(*l1, *r1) && Self::const_eq(*l2, *r2)
            },
            (Self::Star(l), Self::Star(r)) => unsafe { Self::const_eq(*l, *r) },
            _ => false,
        }
    }
}

pub struct Regex<'parent> {
    // We require each Regex to point to something for the head. Regexes can be moved around, so it
    // creates serious complications otherwise.
    tree: Const<Re>,
    alloc: VecAlloc<Re>,
    phantom: PhantomData<&'parent ()>,
}

impl fmt::Debug for Re {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn fmt_rec(r: &Re, unit: bool) -> String {
            match (r, unit) {
                (Re::Zero, _) => format!("0"),
                (Re::One, _) => format!("1"),
                (Re::Char(c), _) => format!("{:?}", c),
                (Re::Seq(r1, r2), _) => unsafe {
                    format!(
                        "{}.{}",
                        fmt_rec(&r1.read(), false),
                        fmt_rec(&r2.read(), false)
                    )
                },
                (Re::Alt(r1, r2), false) => unsafe {
                    format!(
                        "({}|{})",
                        fmt_rec(
                            &r1.read(),
                            if let Re::Alt(..) = r1.read() {
                                true
                            } else {
                                false
                            }
                        ),
                        fmt_rec(
                            &r2.read(),
                            if let Re::Alt(..) = r2.read() {
                                true
                            } else {
                                false
                            }
                        )
                    )
                },
                (Re::Alt(r1, r2), true) => unsafe { format!("{:?}|{:?}", r1.read(), r2.read()) },
                (Re::Star(r), _) => unsafe {
                    match r.read() {
                        Re::Seq(..) | Re::Star(_) => {
                            format!("({:?})*", r.read())
                        }
                        _ => {
                            format!("{:?}*", r.read())
                        }
                    }
                },
            }
        }

        write!(f, "{}", fmt_rec(self, true))
    }
}

impl fmt::Debug for Regex<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regex({:?})", unsafe { self.tree.as_ref() },)
    }
}

impl<'a> From<&build_plan::Re> for Regex<'a> {
    fn from(value: &build_plan::Re) -> Self {
        fn build_inner(
            alloc: &mut VecAlloc<Re>,
            root: &build_plan::Re,
            build_plan: &build_plan::Re,
        ) -> Const<Re> {
            let try_alloc = |alloc: &mut VecAlloc<Re>, value: Re| {
                alloc.alloc(value).map_or_else(
                    |_| build_inner(alloc.resized(), root, root),
                    |v| Const::new(v),
                )
            };

            match build_plan {
                build_plan::Re::One => try_alloc(alloc, Re::One),
                build_plan::Re::Zero => try_alloc(alloc, Re::Zero),
                build_plan::Re::Char(c) => try_alloc(alloc, Re::Char(*c)),
                build_plan::Re::Alt(r1, r2) => {
                    let r1 = build_inner(alloc, root, r1.as_ref());
                    let r2 = build_inner(alloc, root, r2.as_ref());
                    try_alloc(alloc, Re::Alt(r1, r2))
                }
                build_plan::Re::Seq(r1, r2) => {
                    let r1 = build_inner(alloc, root, r1.as_ref());
                    let r2 = build_inner(alloc, root, r2.as_ref());
                    try_alloc(alloc, Re::Seq(r1, r2))
                }
                build_plan::Re::Star(r) => {
                    let r = Re::Star(build_inner(alloc, root, r.as_ref()));
                    try_alloc(alloc, r)
                }
            }
        }

        // SAFETY: the allocator is allocated to using valid methods and all references are dropped
        // on resizes.
        let mut alloc = VecAlloc::new(Regex::DEFAULT_CAPACITY);
        // SAFETY:
        // - the tree is owned by this Regex's allocator, so it's fine.
        //
        // Obviously we have one 'redundany' entry in our allocator, but we'll have to live with it
        let tree = build_inner(&mut alloc, value, value);
        unsafe { Regex::new(tree, alloc) }
    }
}

fn try_alloc(alloc: &mut VecAlloc<Re>, value: Re) -> Result<Const<Re>, ()> {
    alloc.alloc(value).map(|v| Const::new(v)).map_err(|_| ())
}

impl<'a> Regex<'a> {
    pub const DEFAULT_CAPACITY: usize = 32;

    /// SAFETY: not unsafe, but marked as unsafe since `tree` must be owned by `alloc` for most
    /// methods to be sound.
    unsafe fn new(tree: Const<Re>, alloc: VecAlloc<Re>) -> Self {
        Self {
            tree,
            alloc,
            phantom: PhantomData,
        }
    }

    pub fn alloc(&self) -> &VecAlloc<Re> {
        &self.alloc
    }

    /// Exposes unsafe access to the internal allocator. Mutating the internal allocator could
    /// leave references into this Regex dangling.
    pub unsafe fn alloc_mut(&mut self) -> &mut VecAlloc<Re> {
        &mut self.alloc
    }

    /// Exposes unsafe access to the internal tree root. This does not have any lifetime guards,
    /// and so you can link this to anything you want, which is probably unsound. Just don't use it.
    pub unsafe fn tree_mut(&mut self) -> &mut Const<Re> {
        &mut self.tree
    }

    /// Produces a child `Regex`. This Regex is tied to its parent. It is likely not useful.
    pub fn child(&'a self) -> Regex<'a> {
        Regex {
            tree: self.tree.clone(),
            alloc: VecAlloc::new(0),
            phantom: PhantomData,
        }
    }

    /// Checks if this `Regex` is 'nullable'. This means that the regex has consumed enough
    /// characters to be marked as 'complete'.
    pub fn nullable(&self) -> bool {
        // SAFETY: Creates a temporary reference to run a method that returns no owned data.
        unsafe { self.tree.as_ref() }.nullable()
    }

    /// Copies `r` into `alloc`.
    /// SAFETY: `alloc` must not own `r`. `r` must be valid for reads and live for the duration
    /// of the function.
    unsafe fn rebuild_with(alloc: &mut VecAlloc<Re>, r: Const<Re>) -> Const<Re> {
        unsafe fn rebuild_with_rec(
            alloc: &mut VecAlloc<Re>,
            r: Const<Re>,
            root: Const<Re>,
        ) -> Result<Const<Re>, ()> {
            let r = r.read();
            match r {
                Re::Zero | Re::One | Re::Char(_) => try_alloc(alloc, r),
                Re::Alt(r1, r2) => {
                    let r1 = rebuild_with_rec(alloc, r1, root)?;
                    let r2 = rebuild_with_rec(alloc, r2, root)?;
                    try_alloc(alloc, Re::Alt(r1, r2))
                }
                Re::Seq(r1, r2) => {
                    let r1 = rebuild_with_rec(alloc, r1, root)?;
                    let r2 = rebuild_with_rec(alloc, r2, root)?;
                    try_alloc(alloc, Re::Seq(r1, r2))
                }
                Re::Star(r) => {
                    let r = rebuild_with_rec(alloc, r, root)?;
                    try_alloc(alloc, Re::Star(r))
                }
            }
        }

        match rebuild_with_rec(alloc, r, r) {
            Ok(r) => r,
            Err(_) => Self::rebuild_with(alloc.resized(), r),
        }
    }

    /// Completely clone the regex, taking ownership of it. This clone, performs a recursive
    /// search of the actual tree. Cloning a `Regex<'static>` can be done with clone_static
    /// instead, which performs a copy of the internal buffer and is much faster.
    pub fn clone(&self) -> Regex<'static> {
        let mut alloc = VecAlloc::new(self.alloc.capacity());
        let tree = unsafe { Self::rebuild_with(&mut alloc, self.tree) };

        Regex {
            tree,
            alloc,
            phantom: PhantomData,
        }
    }

    /// SAFETY:
    /// - `tree` must be a pointer to the root of a *different* Regex, aka NOT owned by `alloc`.
    /// - On the first recursive call to this function, `r` must be equal to `tree `
    unsafe fn der_rec<'b>(
        tree: Const<Re>,
        alloc: &mut VecAlloc<Re>,
        r: Const<Re>,
        c: char,
    ) -> Result<Const<Re>, ()> {
        match r.as_ref() {
            Re::Zero => Ok(r),
            Re::One => try_alloc(alloc, Re::Zero),
            Re::Char(d) => try_alloc(alloc, if c == *d { Re::One } else { Re::Zero }),
            Re::Alt(r1, r2) => {
                let r = Re::Alt(
                    Self::der_rec(tree, alloc, *r1, c)?,
                    Self::der_rec(tree, alloc, *r2, c)?,
                );
                try_alloc(alloc, r)
            }
            Re::Seq(r1, r2) => {
                let r = if r1.as_ref().nullable() {
                    // der(r1).r2 | der(r2)
                    let tmp = Re::Seq(Self::der_rec(tree, alloc, *r1, c)?, *r2);
                    Re::Alt(try_alloc(alloc, tmp)?, Self::der_rec(tree, alloc, *r2, c)?)
                } else {
                    Re::Seq(Self::der_rec(tree, alloc, *r1, c)?, *r2)
                };
                try_alloc(alloc, r)
            }
            Re::Star(r1) => {
                let r = Re::Seq(Self::der_rec(tree, alloc, *r1, c)?, r);
                try_alloc(alloc, r)
            }
        }
    }

    // Produce the 'derivative' of this regex. The derivative is returned as a 'child', which means
    // that it uses parts of `self` internally to reduce the need for some allocations and
    // hopefully result in less `realloc`s on the internal buffer.
    pub fn der<'b>(&'b self, c: char) -> Regex<'b> {
        let mut alloc = VecAlloc::new(Self::DEFAULT_CAPACITY);
        let tree = loop {
            match unsafe { Self::der_rec(self.tree, &mut alloc, self.tree, c) } {
                Ok(tree) => break tree,
                Err(_) => alloc.resize(),
            }
        };

        Self {
            // SAFETY: `tree` is a valid pointer into `alloc` which we take ownership of.
            tree,
            alloc,
            phantom: PhantomData::default(),
        }
    }

    unsafe fn simp_rec(
        tree: Const<Re>,
        alloc: &mut VecAlloc<Re>,
        r: Const<Re>,
    ) -> Result<Const<Re>, ()> {
        // This is a little tough to understand why we only need to allocate so rarely.
        // Consider something like this:
        //
        // Alt(Char('a'), Alt('a', Star('a'))
        //     \      /   \                /
        //      +----+     +--------------+
        //        |                |
        //    &Char('a')  &Alt('a', Star('a'))
        //
        // In this case, no simplification is required, and we can return a pointer to the root.
        // We know this because r1 == simp(r1) && r2 == simp(r2).
        // Now consider this:
        //
        // Alt(Char('a'), Alt(Zero, Star('a'))
        //     \      /   \                 /
        //      +----+     +---------------+
        //        |                |
        //    &Char('a')      &Star('a')
        //
        // Here, we have to allocate exactly once, which is to create a new `Alt`, since our
        // condition does not hold. We only need to allocate when
        //
        // - sub nodes change during simplification
        // - new nodes are created (e.g. converting from one node type to another)
        match r.as_ref() {
            Re::Alt(r1s, r2s) => unsafe {
                let r1 = Self::simp_rec(tree, alloc, *r1s)?;
                let r2 = Self::simp_rec(tree, alloc, *r2s)?;
                match (r1.as_ref(), r2.as_ref()) {
                    (Re::Zero, _) => Ok(r2),
                    (_, Re::Zero) => Ok(r1),
                    (r1a, r2a) => {
                        if r1a.eq(&r2a) {
                            Ok(r1)
                        } else {
                            if Re::const_eq(r1, *r1s) && Re::const_eq(r2, *r2s) {
                                Ok(r)
                            } else {
                                try_alloc(alloc, Re::Alt(r1, r2))
                            }
                        }
                    }
                }
            },
            Re::Seq(r1s, r2s) => unsafe {
                let r1 = Self::simp_rec(tree, alloc, *r1s)?;
                let r2 = Self::simp_rec(tree, alloc, *r2s)?;
                match (r1.as_ref(), r2.as_ref()) {
                    (Re::Zero, _) => Ok(r1),
                    (_, Re::Zero) => Ok(r2),
                    (Re::One, _) => Ok(r2),
                    (_, Re::One) => Ok(r1),
                    _ => {
                        if Re::const_eq(r1, *r1s) && Re::const_eq(r2, *r2s) {
                            Ok(r)
                        } else {
                            try_alloc(alloc, Re::Seq(r1, r2))
                        }
                    }
                }
            },
            _ => Ok(r),
        }
    }

    pub fn simp<'b>(&'b self) -> Regex<'b> {
        let mut alloc = VecAlloc::new(Self::DEFAULT_CAPACITY);
        let tree = loop {
            match unsafe { Self::simp_rec(self.tree, &mut alloc, self.tree) } {
                Ok(tree) => break tree,
                Err(_) => alloc.resize(),
            }
        };

        Self {
            // SAFETY: `tree` is a valid pointer into `alloc` which we take ownership of.
            tree,
            alloc,
            phantom: PhantomData::default(),
        }
    }

    fn ders(r: Regex<'static>, cs: &[char]) -> Regex<'static> {
        // SAFETY: dereferencing a reference to immutable buffers
        if let Re::Zero = unsafe { r.tree.as_ref() } {
            return r;
        }
        match cs {
            [] => r,
            [c1, c2, c3, c4, cs @ ..] => {
                let c1 = r.der(*c1);
                let c1s = c1.simp();
                let c2 = c1s.der(*c2);
                let c2s = c2.simp();
                let c3 = c2s.der(*c3);
                let c3s = c3.simp();
                let c4 = c3s.der(*c4);
                let c4s = c4.simp();
                Regex::ders(c4s.clone(), cs)
            }
            [c, cs @ ..] => Regex::ders(r.der(*c).simp().clone(), cs),
        }
    }

    pub fn is_match(&self, s: &str) -> bool {
        let d = Regex::ders(self.clone(), &s.chars().collect::<Vec<char>>());
        d.nullable()
    }
}

impl Regex<'static> {
    pub fn clone_static(&self) -> Self {
        unimplemented!("Use `clone` for now")
    }
}
