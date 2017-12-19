use std::sync::Arc;

use backpat::{parse, compile};
use backpat::parse::{TokenStream, Error};

use ident::*;
use token::*;

pub type Ast = parse::Ast<Var<Ident>>;
pub type Expr = Arc<parse::Ast<Var<usize>>>;
pub type Pattern = Arc<compile::Compiled>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Var<Local> {
    Local { name: Local, },
    Global { name: Ident, },
}

impl<'a> TokenStream<Var<Ident>> for Tokenizer<'a> {
    fn getc(&mut self) -> Option<char> {
        Tokenizer::getc(self)
    }

    fn lookahead(&mut self) -> Option<char> {
        Tokenizer::lookahead(self)
    }

    fn parse_payload(&mut self, sigil: char) -> Result<Var<Ident>, Error> {
        if let Some(Ok(name)) = self.word() {
            match sigil {
                '$' => Ok(Var::Local { name }),
                '%' => Ok(Var::Global { name }),
                _ => unreachable!("No such sigil: {:?}", sigil),
            }
        } else {
            Err(Error::Bad)
        }
    }
}

mod display {
    use super::*;
    use std::fmt::{Display, Formatter, Result};

    impl Display for Var<Ident> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Var::Local { ref name } => write!(f, "${}", name),
                Var::Global { ref name } => write!(f, "%{}", name),
            }
        }
    }
}
