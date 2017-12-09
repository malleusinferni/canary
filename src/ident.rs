use super::*;
use value::Str;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Ident(Str);

impl Ident {
    pub fn new<S>(name: S) -> Result<Self> where S: Into<Str> {
        let name = name.into();

        let copy = name.clone();
        let die = || Err(Error::InvalidIdent {
            input: copy.as_ref().to_owned()
        });

        match name.chars().next() {
            Some(ch) if ch.is_alphabetic() => (),
            _ => die()?,
        }

        for ch in name.chars().skip(1) {
            if ch.is_alphabetic() || ch.is_digit(10) || ch == '_' {
                continue;
            }

            die()?;
        }

        Ok(Ident(name))
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

use std::fmt;

impl fmt::Display for Ident {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.0)
    }
}
