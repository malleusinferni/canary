extern crate failure;

#[macro_use]
extern crate failure_derive;

extern crate lalrpop_util;

pub mod ident;
pub mod token;
pub mod ast;
pub mod value;
pub mod opcode;
pub mod eval;

use std::path::Path;

//use value::*;
use ident::*;
use token::Token;

pub fn compile(path: &Path) -> Result<eval::World> {
    use std::fs::File;
    use std::io::Read;

    let mut source = String::new();
    File::open(path)?.read_to_string(&mut source)?;

    let tokens = token::Tokenizer::new(&source).spanned();
    let ast = ast::parse_module(tokens)?;
    let program = ast::translate(ast)?;
    Ok(eval::World::new(program))
}

#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display="stack underflow")]
    StackUnderflow,

    #[fail(display="{} was called with {} arguments, wanted {}", func, expected, found)]
    WrongArgc {
        func: Ident,
        expected: opcode::Argc,
        found: usize,
    },

    #[fail(display="expected {}, found {}", expected, found)]
    TypeMismatch { expected: &'static str, found: &'static str, },

    #[fail(display="invalid identifier {:?}", input)]
    InvalidIdent { input: String, },

    #[fail(display="incorrect indentation")]
    IncorrectIndent,

    #[fail(display="token cannot start with {:?}", ch)]
    UnimplementedToken { ch: char, },

    #[fail(display="divided by zero")]
    DividedByZero,

    #[fail(display="negative repetition")]
    NegativeRepetition,

    #[fail(display="ok")]
    Okay,

    #[fail(display="{}", parse)]
    Parse {
        parse: Box<lalrpop_util::ParseError<usize, Token, Error>>,
    },

    #[fail(display="{}", io)]
    Io {
        io: std::io::Error,
    },
}

pub type Result<T, E=Error> = std::result::Result<T, E>;

impl From<std::io::Error> for Error {
    fn from(io: std::io::Error) -> Self {
        Error::Io { io }
    }
}

impl From<lalrpop_util::ParseError<usize, Token, Error>> for Error {
    fn from(parse: lalrpop_util::ParseError<usize, Token, Error>) -> Self {
        let parse = Box::new(parse);
        Error::Parse { parse }
    }
}

#[test]
fn use_value() {
    use value::*;

    assert_eq!(Value::Int(1), Value::Int(1));
}
