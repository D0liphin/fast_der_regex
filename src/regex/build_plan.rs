/// Maps one-to-one with `regex::Re`, but provides a safe way of constructing proper `Regex`
pub enum Re {
    One,
    Zero,
    Char(char),
    Alt(Box<Re>, Box<Re>),
    Seq(Box<Re>, Box<Re>),
    Star(Box<Re>),
}

impl Re {
    pub fn char(c: char) -> Self {
        Self::Char(c)
    }
}

pub trait ImplicitRe: Into<Re> {
    fn into_boxed(self) -> Box<Re> {
        <Self as Into<Re>>::into(self).into()
    }

    fn re(self) -> Re {
        self.into()
    }

    fn alt(self, rhs: impl ImplicitRe) -> Re {
        Re::Alt(self.into_boxed(), rhs.into_boxed())
    }

    fn seq(self, rhs: impl ImplicitRe) -> Re {
        Re::Seq(self.into_boxed(), rhs.into_boxed())
    }

    fn star(self) -> Re {
        Re::Star(self.into_boxed())
    }
}

impl Into<Re> for char {
    fn into(self) -> Re {
        Re::Char(self)
    }
}

impl<'a> Into<Re> for &'a str {
    fn into(self) -> Re {
        if self.len() == 0 {
            return Re::One;
        }

        let mut iter = self.chars();
        let mut r: Re = iter.next().unwrap().into();
        for c in iter {
            r = r.seq(c)
        }

        r
    }
}

impl ImplicitRe for Re {}
impl ImplicitRe for char {}
impl<'a> ImplicitRe for &'a str {}
