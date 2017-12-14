pub mod parse;

//use value::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Deferred(parse::Ast),
}

impl Pattern {
    pub fn matches(self, _haystack: &str) -> bool {
        match self {
            Pattern::Deferred(_ast) => {
                unimplemented!()
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
