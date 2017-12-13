use std::collections::HashSet;
use std::borrow::Borrow;

use super::*;
use value::Str;

#[derive(Clone, Debug)]
pub struct Strings(HashSet<Str>);

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ident(Str);

pub trait Interned: Sized {
    fn from_arc(&Str) -> Result<Self>;
}

impl Strings {
    pub fn new() -> Self {
        Strings(HashSet::new())
    }

    pub fn intern<I, O>(&mut self, input: I) -> Result<O>
        where I: AsRef<str>, O: Interned
    {
        let input = input.as_ref();

        if !self.0.contains(input) {
            self.0.insert(input.into());
        }

        O::from_arc(self.0.get(input).expect("We just inserted this"))
    }
}

impl Interned for Str {
    fn from_arc(arc: &Str) -> Result<Self> {
        Ok(arc.clone())
    }
}

impl Interned for Ident {
    fn from_arc(arc: &Str) -> Result<Self> {
        let die = || {
            let input = String::from(arc.as_ref());
            Err(Error::InvalidIdent { input })
        };

        let mut chars = arc.chars();
        match chars.next() {
            Some(ch) if ch.is_alphabetic() => (),
            _ => die()?,
        };

        for ch in chars {
            if ch.is_alphabetic() || ch.is_digit(10) || ch == '_' {
                continue;
            }

            die()?;
        }

        Ok(Ident(arc.clone()))
    }
}

impl From<Ident> for String {
    fn from(Ident(s): Ident) -> Self {
        s.as_ref().to_owned()
    }
}

impl AsRef<str> for Ident {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl Borrow<str> for Ident {
    fn borrow(&self) -> &str {
        self.0.as_ref()
    }
}

use std::fmt;

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}
