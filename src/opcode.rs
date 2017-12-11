use std::collections::{HashMap};
use std::sync::Arc;

use super::*;
use ident::*;
use value::*;

pub struct Program {
    pub code: Vec<Op<Ident>>,
    pub labels: HashMap<Ident, usize>,
    pub functions: HashMap<Ident, (Argc, Func)>,
}

pub type NativeFn = Arc<Fn(Vec<Value>) -> Result<Value>>;

#[derive(Clone)]
pub enum Func {
    Native(NativeFn),
    Label(usize),
}

#[derive(Copy, Clone, Debug)]
pub enum Argc {
    Exactly(usize),
    AtLeast(usize),
}

#[derive(Clone, Debug)]
pub enum Op<Label=usize> {
    RET,
    DROP,
    NOT,
    NIL,
    CALL { name: Ident, argc: usize, },
    BINOP { op: Binop, },
    LOAD { src: usize, },
    STORE { dst: usize, },
    GLOBALS,
    INS,
    PUSHI { int: Int, },
    PUSHS { string: Str, },
    PUSHN { name: Ident, },
    LIST { len: usize, },
    REC,
    JUMP { dst: Label, },
    JNZ { dst: Label, },
}

#[derive(Copy, Clone, Debug)]
pub enum Binop {
    ADD,
    SUB,
    DIV,
    MUL,
    IDX,
}

impl Program {
    pub fn fetch(&self, pc: usize) -> Result<Op<Ident>> {
        self.code.get(pc).cloned().ok_or(Error::IndexOutOfBounds)
    }

    pub fn jump(&self, label: Ident) -> Result<usize> {
        self.labels.get(&label).cloned().ok_or(Error::NoSuchLabel)
    }

    pub fn call(&self, name: Ident, argv: &[Value]) -> Result<Func> {
        let (wanted, func) = self.functions.get(&name).cloned()
            .ok_or(Error::NoSuchLabel)?;

        match wanted {
            Argc::Exactly(argc) if argc == argv.len() => Ok(func),

            Argc::AtLeast(argc) if argc <= argv.len() => {
                Ok(func)
            },

            expected => Err(Error::WrongArgc {
                expected,
                func: name,
                found: argv.len(),
            }),
        }
    }
}

