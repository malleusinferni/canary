use std::iter::FromIterator;

use super::*;
use value::*;
use opcode::*;

pub struct Interpreter {
    main: Module,
    strings: Strings,
    globals: Record,
    frame: Frame,
    saved: Vec<Frame>,
}

struct Frame {
    code: InterpretedFn,
    locals: Vec<Value>,
    pc: usize,
}

impl Interpreter {
    pub fn new(main: Module) -> Result<Self> {
        let mut this = Interpreter {
            frame: Frame {
                code: main.begin.clone(),
                locals: vec![],
                pc: 0,
            },

            main,
            strings: Strings::new(),
            globals: Record::default(),
            saved: vec![],
        };

        while this.frame.pc < this.frame.code.len() {
            this.step()?;
        }

        Ok(this)
    }

    pub fn exec(&mut self, func: &str, args: &[Value]) -> Result<Value> {
        let func = self.strings.intern(func)?;
        self.fncall(&func, args.to_owned())?;

        while self.saved.len() > 0 {
            self.step()?;
        }

        self.pop()
    }

    pub fn step(&mut self) -> Result<()> {
        let op = self.frame.code.fetch(self.frame.pc)?;

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

            Op::NIL => {
                self.push(());
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
                let lhs = self.pop::<Value>()?;
                let idx = self.pop::<Value>()?;
                let rhs = self.pop::<Value>()?;
                lhs.insert(idx, rhs)?;
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
                self.frame.pc = dst;
            },

            Op::JNZ { dst } => {
                let test = self.pop::<Int>()?;

                if test != 0 {
                    self.frame.pc = dst;
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
        match self.main.call(name.clone(), &argv)? {
            Func::Native(call) => {
                // Immediately call it and save the return value
                self.push(call(argv)?);
            },

            Func::Interpreted(code) => {
                use std::mem::swap;

                let locals = argv;
                let pc = 0;
                self.saved.push(Frame { locals, pc, code });

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
            self.frame.locals.get(index).cloned()
                .ok_or(Error::IndexOutOfBounds)?
        })
    }

    fn write<V: Into<Value>>(&mut self, item: V, index: usize) -> Result<()> {
        if index >= self.frame.locals.len() {
            Err(Error::IndexOutOfBounds)
        } else {
            Ok({ self.frame.locals[index] = item.into() })
        }
    }

    fn capture<O: FromIterator<Value>>(&mut self, len: usize) -> Result<O> {
        let start = self.frame.locals.len().checked_sub(len)
            .ok_or(Error::IndexOutOfBounds)?;
        Ok(self.frame.locals.drain(start ..).collect())
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
