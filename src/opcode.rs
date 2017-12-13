use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use ident::*;
use value::*;
use pattern::*;

pub struct Module {
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
    DUP,
    DROP,
    NOT,
    NIL,
    CALL { name: Ident, argc: usize, },
    BINOP { op: Binop, },
    LOAD { src: usize, },
    STORE { dst: usize, },
    GROUP { num: u8, },
    GLOBALS,
    INS,
    PUSHI { int: Int, },
    PUSHS { string: Str, },
    PUSHN { name: Ident, },
    PAT { pat: Pattern, },
    LIST { len: usize, },
    STR { len: usize, },
    REC,
    JUMP { dst: Label, },
    JNZ { dst: Label, },
    MARK { len: usize, },
}

#[derive(Copy, Clone, Debug)]
pub enum Binop {
    ADD,
    SUB,
    DIV,
    MUL,
    IDX,
    MATCH,
    EQ,
    NE,
}

impl Module {
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
        self.0.get(pc).cloned().ok_or(Error::PcOutOfBounds { pc })
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}
