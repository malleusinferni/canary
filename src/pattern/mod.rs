pub mod parse;
pub mod compile;

use ident::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Resolved(parse::Ast<usize>),
}

impl Pattern {
    pub fn matches(self, _haystack: &str) -> bool {
        match self {
            Pattern::Resolved(_ast) => {
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
