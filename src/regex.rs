use std::{collections::VecDeque, fmt, marker::PhantomData, pin::Pin, ptr::NonNull};

use crate::vec_alloc::VecAlloc;

/// This class is only meant so that I can make sure that I am not using any mutating methods on
/// the internal pointer. It still needs to be constructed from some kind of mutable pointer.
#[derive(Clone, Debug, PartialEq, Copy)]
pub struct Const<T>(NonNull<T>);

impl<T> Const<T> {
    pub fn new(inner: NonNull<T>) -> Self {
        Self(inner)
    }

    pub unsafe fn read(&self) -> T {
        self.0.as_ptr().read()
    }

    pub unsafe fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> From<NonNull<T>> for Const<T> {
    fn from(value: NonNull<T>) -> Self {
        Self::new(value)
    }
}

impl<T> From<&T> for Const<T> {
    fn from(value: &T) -> Self {
        Self::new(value.into())
    }
}

#[derive(Clone, Copy)]
pub enum Re {
    Zero,
    One,
    Char(char),
    Alt(Const<Re>, Const<Re>),
    Seq(Const<Re>, Const<Re>),
    Star(Const<Re>),
}

impl PartialEq for Re {
    fn eq(&self, other: &Self) -> bool {
        unsafe fn equals(lhs: &Const<Re>, rhs: &Const<Re>) -> bool {
            lhs == rhs || lhs.read() == rhs.read()
        }

        match (self, other) {
            (Self::Zero, Self::Zero) => true,
            (Self::One, Self::One) => true,
            (Self::Char(c), Self::Char(d)) => c == d,
            (Self::Alt(l1, l2), Self::Alt(r1, r2)) => unsafe { equals(l1, r1) && equals(l2, r2) },
            (Self::Seq(l1, l2), Self::Seq(r1, r2)) => unsafe { equals(l1, r1) && equals(l2, r2) },
            (Self::Star(l), Self::Star(r)) => unsafe { equals(l, r) },
            _ => false,
        }
    }
}

impl Re {
    // TODO: make #[tailcall]
    pub fn nullable(&self) -> bool {
        match &self {
            Re::Zero => false,
            Re::One => true,
            Re::Char(_) => false,
            Re::Alt(r1, r2) => unsafe { r1.read().nullable() || r2.read().nullable() },
            Re::Seq(r1, r2) => unsafe { r1.read().nullable() && r2.read().nullable() },
            Re::Star(_) => true,
        }
    }
}

pub struct Regex<'parent> {
    tree: Re,
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
        write!(f, "Regex({:?})", self.tree)
    }
}

impl<'a> Regex<'a> {
    pub const DEFAULT_CAPACITY: usize = 16;

    pub unsafe fn alloc_mut(&mut self) -> &mut VecAlloc<Re> {
        &mut self.alloc
    }

    pub unsafe fn tree_mut(&mut self) -> &mut Re {
        &mut self.tree
    }

    pub fn child(&'a self) -> Regex<'a> {
        Regex {
            tree: self.tree.clone(),
            alloc: VecAlloc::new(0),
            phantom: PhantomData::default(),
        }
    }

    pub fn new() -> Self {
        Self {
            tree: Re::Zero,
            alloc: VecAlloc::new(Self::DEFAULT_CAPACITY),
            phantom: PhantomData::default(),
        }
    }

    pub fn nullable(&self) -> bool {
        self.tree.nullable()
    }

    fn rebuild_from(&mut self, r: Re) -> Const<Re> {
        let alloc = |alloc: &mut VecAlloc<Re>, r| alloc.alloc(r).unwrap().into();
        unsafe {
            match r {
                Re::Zero | Re::One | Re::Char(_) => alloc(&mut self.alloc, r),
                Re::Alt(r1, r2) => {
                    let r1 = self.rebuild_from(r1.read());
                    let r2 = self.rebuild_from(r2.read());
                    alloc(&mut self.alloc, Re::Alt(r1, r2))
                }
                Re::Seq(r1, r2) => {
                    let r1 = self.rebuild_from(r1.read());
                    let r2 = self.rebuild_from(r2.read());
                    alloc(&mut self.alloc, Re::Seq(r1, r2))
                }
                Re::Star(r) => {
                    let r = self.rebuild_from(r.read());
                    alloc(&mut self.alloc, Re::Star(r))
                }
            }
        }
    }

    pub fn to_owned(&self) -> Regex<'static> {
        let mut new = Regex {
            tree: Re::Zero,
            alloc: VecAlloc::new(self.alloc.capacity()),
            phantom: PhantomData::<&'static ()>::default(),
        };

        new.tree = unsafe { new.rebuild_from(self.tree).read() };
        new
    }

    fn der_alloc<'b>(tree: Const<Re>, alloc: &mut VecAlloc<Re>, r: Re, c: char) -> Const<Re> {
        match alloc.alloc(r) {
            Ok(ptr) => ptr.into(),
            Err(_) => {
                println!("failed allocation of Re node. alloc = {:?}", alloc);
                Self::der_rec(tree, alloc.resized(), tree.into(), c)
            }
        }
    }

    fn der_rec<'b>(tree: Const<Re>, alloc: &mut VecAlloc<Re>, r: Const<Re>, c: char) -> Const<Re> {
        let try_alloc = move |alloc: &mut VecAlloc<Re>, r: Re| Self::der_alloc(tree, alloc, r, c);

        let r = match unsafe { r.read() } {
            Re::Zero => Re::Zero,
            Re::One => Re::Zero,
            Re::Char(d) => {
                if c == d {
                    Re::One
                } else {
                    Re::Zero
                }
            }
            Re::Alt(r1, r2) => Re::Alt(
                Self::der_rec(tree, alloc, r1, c),
                Self::der_rec(tree, alloc, r2, c),
            ),
            Re::Seq(r1, r2) => unsafe {
                if r1.read().nullable() {
                    let tmp = Re::Seq(Self::der_rec(tree, alloc, r1, c), r2);
                    Re::Alt(try_alloc(alloc, tmp), Self::der_rec(tree, alloc, r2, c))
                } else {
                    Re::Seq(Self::der_rec(tree, alloc, r1, c), r2)
                }
            },
            Re::Star(r) => Re::Seq(
                Self::der_rec(tree, alloc, r, c),
                try_alloc(alloc, Re::Star(r)),
            ),
            _ => todo!(),
        };

        try_alloc(alloc, r)
    }

    pub fn der<'b>(&'b self, c: char) -> Regex<'b> {
        let mut alloc = VecAlloc::new(Self::DEFAULT_CAPACITY);
        // It is important that we reallocate the root of the tree. Otherwise, we could run into a
        // situation where we use a `Const<Re>` to it internally in the returned derivative, which
        // would become invalid after we move the parent.
        let tree = alloc.alloc(self.tree).unwrap().into();
        let tree = Self::der_rec(tree, &mut alloc, tree, c);

        Self {
            tree: unsafe { tree.read() },
            alloc,
            phantom: PhantomData::default(),
        }
    }

    fn simp_alloc<'b>(tree: Const<Re>, alloc: &mut VecAlloc<Re>, r: Re) -> Const<Re> {
        match alloc.alloc(r) {
            Ok(ptr) => ptr.into(),
            Err(_) => {
                println!("failed allocation of Re node. alloc = {:?}", alloc);
                Self::simp_rec(tree, alloc.resized(), tree)
            }
        }
    }

    fn simp_rec(tree: Const<Re>, alloc: &mut VecAlloc<Re>, r: Const<Re>) -> Const<Re> {
        let try_alloc = move |alloc: &mut VecAlloc<Re>, r: Re| Self::simp_alloc(tree, alloc, r);

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
        match unsafe { r.read() } {
            Re::Alt(r1s, r2s) => unsafe {
                let r1 = Self::simp_rec(tree, alloc, r1s);
                let r2 = Self::simp_rec(tree, alloc, r2s);
                match (r1.read(), r2.read()) {
                    (Re::Zero, _) => r2,
                    (_, Re::Zero) => r1,
                    (r1a, r2a) => {
                        if r1a == r2a {
                            r1
                        } else {
                            if (r1, r2) == (r1s, r2s) {
                                r
                            } else {
                                try_alloc(alloc, Re::Alt(r1, r2))
                            }
                        }
                    }
                }
            },
            Re::Seq(r1s, r2s) => unsafe {
                let r1 = Self::simp_rec(tree, alloc, r1s);
                let r2 = Self::simp_rec(tree, alloc, r2s);
                match (r1.read(), r2.read()) {
                    (Re::Zero, _) => r1,
                    (_, Re::Zero) => r2,
                    (Re::One, _) => r2,
                    (_, Re::One) => r1,
                    _ => {
                        if (r1, r2) == (r1s, r2s) {
                            r
                        } else {
                            try_alloc(alloc, Re::Seq(r1, r2))
                        }
                    }
                }
            },
            _ => r,
        }
    }

    pub fn simp(&'a self) -> Regex<'a> {
        let mut alloc = VecAlloc::new(Self::DEFAULT_CAPACITY);
        let tree = alloc.alloc(self.tree).unwrap().into();
        let tree = Self::simp_rec(tree, &mut alloc, tree);

        Self {
            tree: unsafe { tree.read() },
            alloc,
            phantom: PhantomData::default(),
        }
    }

    fn ders(r: Regex<'static>, cs: &[char]) -> Regex<'static> {
        if let Re::Zero = r.tree {
            return Regex::new();
        }
        // TODO: run a number of derivatives at a time
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
                Regex::ders(c4s.to_owned(), cs)
            }
            [c, cs @ ..] => Regex::ders(r.der(*c).simp().to_owned(), cs),
        }
    }

    pub fn is_match(&self, s: &str) -> bool {
        let d = Regex::ders(self.to_owned(), &s.chars().collect::<Vec<char>>());
        d.nullable()
    }
}

