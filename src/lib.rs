extern crate failure;

#[macro_use]
extern crate failure_derive;

extern crate lalrpop_util;

pub mod ident;
pub mod pattern;
pub mod token;
pub mod ast;
pub mod value;
pub mod opcode;
pub mod build;
pub mod eval;

use std::path::Path;

//use value::*;
use ident::*;
use token::Token;

pub fn compile<P: AsRef<Path>>(path: P) -> Result<opcode::Module> {
    use std::fs::File;
    use std::io::Read;

    let mut source = String::new();
    File::open(path.as_ref())?.read_to_string(&mut source)?;

    let tokens = token::Tokenizer::new(&source).spanned();
    ast::parse_module(tokens)?.translate()
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

    #[fail(display="invalid regex")]
    InvalidRegex,

    #[fail(display="incorrect indentation")]
    IncorrectIndent,

    #[fail(display="malformed string")]
    MalformedString,

    #[fail(display="token cannot start with {:?}", ch)]
    UnimplementedToken { ch: char, },

    #[fail(display="illegal lvalue expression")]
    IllegalLvalue,

    #[fail(display="illegal add")]
    IllegalAdd,

    #[fail(display="illegal multiply")]
    IllegalMultiply,

    #[fail(display="divided by zero")]
    DividedByZero,

    #[fail(display="negative repetition")]
    NegativeRepetition,

    #[fail(display="negative index")]
    NegativeIndex,

    #[fail(display="index out of bounds")]
    IndexOutOfBounds,

    #[fail(display="no such label")]
    NoSuchLabel,

    #[fail(display="nested functions are unsupported")]
    NonStaticFunction,

    #[fail(display="internal compiler error")]
    InternalCompilerErr,

    #[fail(display="label redefined")]
    LabelRedefined,

    #[fail(display="variable renamed in same scope")]
    VariableRenamed,

    #[fail(display="variable not defined")]
    VariableUndefined,

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
