use std::collections::{HashSet};

use std::iter::FromIterator;

use super::*;
use value::*;
use opcode::*;

pub struct World {
    program: Program,
    strings: HashSet<Str>,
    globals: Record,
    frame: Frame,
    saved: Vec<Frame>,
}

struct Frame {
    locals: Vec<Value>,
    pc: usize,
}

impl World {
    pub fn new(program: Program) -> Self {
        World {
            program,
            strings: HashSet::new(),
            globals: Record::default(),
            frame: Frame {
                pc: 0,
                locals: vec![],
            },
            saved: vec![],
        }
    }

    pub fn exec(&mut self, func: &str, args: &[Value]) -> Result<Value> {
        let func = Ident::new(self.intern(func))?;
        self.fncall(&func, args.to_owned())?;

        while self.saved.len() > 0 {
            self.step()?;
        }

        self.pop()
    }

    pub fn step(&mut self) -> Result<()> {
        let op = self.program.fetch(self.frame.pc)?;

        self.frame.pc += 1;

        match op {
            Op::RET => {
                let saved = self.saved.pop()
                    .ok_or(Error::StackUnderflow)?;

                let rv: Value = self.pop()?;

                self.frame = saved;
                self.push(rv);
            },

            Op::DROP => {
                let _: Value = self.pop()?;
            },

            Op::LOAD { src } => {
                let val: Value = self.read(src)?;
                self.push(val);
            },

            Op::STORE { dst } => {
                let val: Value = self.pop()?;
                self.write(val, dst)?;
            },

            Op::GLOBALS => {
                let globals = self.globals.clone();
                self.push(globals);
            },

            Op::PUSHI { int } => {
                self.push(int);
            },

            Op::PUSHS { string } => {
                self.push(string);
            },

            Op::PUSHN { name } => {
                self.push(name);
            },

            Op::NOT => {
                let i: Int = self.pop()?;
                self.push(if i == 0 { 1 } else { 0 })
            },

            Op::BINOP { op } => {
                let rhs = self.pop::<Value>()?;
                let lhs = self.pop::<Value>()?;

                self.push(match op {
                    Binop::ADD => lhs + rhs,
                    Binop::SUB => lhs - rhs,
                    Binop::DIV => lhs / rhs,
                    Binop::MUL => lhs * rhs,
                    Binop::IDX => lhs.index(rhs),
                }?);
            },

            Op::INS => {
                let val = self.pop::<Value>()?;
                let rhs = self.pop::<Value>()?;
                let lhs = self.pop::<Value>()?;
                lhs.insert(rhs, val)?;
            },

            Op::LIST { len } => {
                use std::collections::VecDeque;
                let list: VecDeque<_> = self.capture(len)?;
                self.push(List::new(list.into()));
            },

            Op::REC => {
                self.push(Record::default());
            },

            Op::JUMP { dst } => {
                self.frame.pc = self.program.jump(dst)?;
            },

            Op::JNZ { dst } => {
                let test = self.pop::<Int>()?;

                if test != 0 {
                    self.frame.pc = self.program.jump(dst)?;
                }
            },

            Op::CALL { name, argc } => {
                let mut argv = self.capture(argc)?;
                self.fncall(&name, argv)?;
            },
        }

        Ok(())
    }

    fn fncall(&mut self, name: &Ident, argv: Vec<Value>) -> Result<()> {
        match self.program.call(name.clone(), &argv)? {
            Func::Native(call) => {
                // Immediately call it and save the return value
                self.push(call(argv)?);
            },

            Func::Label(pc) => {
                use std::mem::swap;

                self.saved.push(Frame { locals: argv, pc, });
                swap(&mut self.frame, self.saved.last_mut().unwrap());

                // Return value will be saved by the RET instruction
            },
        }

        Ok(())
    }

    pub fn pop<V: Extract>(&mut self) -> Result<V> {
        Extract::extract({
            self.frame.locals.pop().ok_or(Error::StackUnderflow)?
        })
    }

    pub fn push<V: Into<Value>>(&mut self, item: V) {
        self.frame.locals.push(item.into());
    }

    fn read<V: Extract>(&self, index: usize) -> Result<V> {
        Extract::extract({
            self.frame.locals.get(index).cloned().ok_or(Error::Okay)?
        })
    }

    fn write<V: Into<Value>>(&mut self, item: V, index: usize) -> Result<()> {
        if index >= self.frame.locals.len() {
            Err(Error::Okay)
        } else {
            Ok({ self.frame.locals[index] = item.into() })
        }
    }

    fn capture<O: FromIterator<Value>>(&mut self, len: usize) -> Result<O> {
        let start = self.frame.locals.len().checked_sub(len)
            .ok_or(Error::Okay)?;
        Ok(self.frame.locals.drain(start ..).collect())
    }

    pub fn intern(&mut self, string: &str) -> Str {
        if !self.strings.contains(string) {
            self.strings.insert(string.into());
        }

        self.strings.get(string).cloned()
            .expect("We just inserted this")
    }
}

use std::fmt;

impl fmt::Display for Argc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Argc::AtLeast(n) => write!(f, "at least {}", n),
            Argc::Exactly(n) => write!(f, "exactly {}", n),
        }
    }
}

#[test]
fn hello() {
    use opcode::Assembler;

    let mut asm = Assembler::new();

    asm.push_str("Hello, ");
    asm.push_str("world.");
    asm.call("str", 2).unwrap();
    asm.call("print", 1).unwrap();

    let mut w = World::new(asm.build().unwrap());

    for _ in 0 .. 4 {
        w.step().unwrap();
    }
}