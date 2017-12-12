use value::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Find(Str),
}

impl Pattern {
    pub fn matches(self, haystack: &str) -> bool {
        match self {
            Pattern::Find(needle) => {
                let needle: &str = needle.as_ref();
                haystack.contains(needle)
            }
        }
    }
}

mod display {
    use std::fmt::{Display, Formatter, Result};

    use super::*;

    impl Display for Pattern {
        fn fmt(&self, f: &mut Formatter) -> Result {
            write!(f, "re/.../")
        }
    }
}
