use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use ident::*;
use value::*;

pub struct Program {
    pub begin: InterpretedFn,
    pub functions: HashMap<Ident, (Argc, Func)>,
    pub strings: Strings,
}

pub type NativeFn = Arc<Fn(Vec<Value>) -> Result<Value>>;

#[derive(Clone, Debug)]
pub struct InterpretedFn(Arc<[Op]>);

#[derive(Clone)]
pub enum Func {
    Native(NativeFn),
    Interpreted(InterpretedFn),
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

impl InterpretedFn {
    pub fn from_vec(code: Vec<Op>) -> Self {
        InterpretedFn(code.into())
    }

    pub fn fetch(&self, pc: usize) -> Result<Op> {
        self.0.get(pc).cloned().ok_or(Error::IndexOutOfBounds)
    }
}

