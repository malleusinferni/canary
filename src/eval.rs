use std::iter::FromIterator;

use super::*;
use value::*;
use opcode::*;
use pattern::*;

pub struct Interpreter {
    main: Module,
    strings: Strings,
    globals: Record,
    frame: Frame,
    saved: Vec<Frame>,
}

struct Frame {
    code: InterpretedFn,
    mark: usize,
    locals: Vec<Value>,
    groups: Vec<Value>,
    pc: usize,
}

impl Module {
    pub fn start(self) -> Result<Interpreter> {
        let mut this = Interpreter {
            frame: Frame {
                code: self.begin.clone(),
                locals: vec![],
                groups: vec![],
                mark: 0,
                pc: 0,
            },

            main: self,
            strings: Strings::new(),
            globals: Record::default(),
            saved: vec![],
        };

        while this.frame.pc < this.frame.code.len() {
            this.step()?;
        }

        Ok(this)
    }
}

impl Interpreter {
    pub fn exec(&mut self, func: &str, args: &[Value]) -> Result<Value> {
        let func = self.strings.intern(func)?;
        self.fncall(&func, args.to_owned())?;

        while self.saved.len() > 0 {
            self.step()?;
        }

        self.pop()
    }

    pub fn set_global<V>(&mut self, name: &str, value: V) -> Result<()>
        where V: Into<Value>
    {
        let value = value.into();
        let name: Ident = self.strings.intern(name)?;
        self.globals.borrow_mut().insert(name, value);
        Ok(())
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

            Op::DUP => {
                let val: Value = self.pop()?;
                self.push(val.clone());
                self.push(val);
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

            Op::GROUP { num } => {
                let group = self.frame.groups.get(num as usize).cloned()
                    .ok_or(Error::NoSuchGroup { num })?;
                self.push(group);
            }

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

            Op::PAT { pat } => {
                self.push(pat);
            },

            Op::NOT => {
                let test = self.pop::<bool>()?;
                self.push(!test);
            },

            Op::BINOP { op } => {
                let rhs = self.pop::<Value>()?;
                let lhs = self.pop::<Value>()?;

                let result = match op {
                    Binop::ADD => lhs + rhs,
                    Binop::SUB => lhs - rhs,
                    Binop::DIV => lhs / rhs,
                    Binop::MUL => lhs * rhs,
                    Binop::IDX => lhs.index(rhs),

                    Binop::EQ => Ok((lhs == rhs).into()),
                    Binop::NE => Ok((lhs != rhs).into()),

                    Binop::MATCH => {
                        self.match_pattern(rhs, lhs)
                    },
                }?;

                self.push(result);
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

            Op::STR { len } => {
                let mut buf = String::new();

                let items: Vec<_> = self.capture(len)?;
                for item in items {
                    buf.push_str(&item.to_string());
                }

                self.push(Str::from(buf));
            },

            Op::REC => {
                self.push(Record::default());
            },

            Op::JUMP { dst } => {
                self.frame.pc = dst;
            },

            Op::JNZ { dst } => {
                if self.pop::<bool>()? {
                    self.frame.pc = dst;
                }
            },

            Op::MARK { len } => {
                if len > self.frame.locals.len() {
                    return Err(Error::MarkTooHigh);
                }

                self.frame.mark = len;
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

                self.saved.push(Frame {
                    groups: vec![],
                    mark: argv.len(),
                    locals: argv,
                    pc: 0,
                    code,
                });

                swap(&mut self.frame, self.saved.last_mut().unwrap());

                // Return value will be saved by the RET instruction
            },
        }

        Ok(())
    }

    pub fn pop<V: Extract>(&mut self) -> Result<V> {
        let val = self.frame.locals.pop()
            .ok_or(Error::StackUnderflow)?;

        if self.frame.locals.len() < self.frame.mark {
            return Err(Error::PoppedLocalVar);
        }

        Extract::extract(val)
    }

    pub fn push<V: Into<Value>>(&mut self, item: V) {
        self.frame.locals.push(item.into());
    }

    fn read<V: Extract>(&self, index: usize) -> Result<V> {
        if index >= self.frame.mark {
            return Err(Error::LocalVarOutOfBounds { index });
        }

        Extract::extract(self.frame.locals[index].clone())
    }

    fn write<V: Into<Value>>(&mut self, item: V, index: usize) -> Result<()> {
        if index >= self.frame.mark {
            Err(Error::LocalVarOutOfBounds { index })
        } else {
            Ok({ self.frame.locals[index] = item.into() })
        }
    }

    fn capture<O: FromIterator<Value>>(&mut self, len: usize) -> Result<O> {
        let start = self.frame.locals.len().checked_sub(len)
            .ok_or(Error::ListTooLong)?;

        Ok(self.frame.locals.drain(start ..).collect())
    }

    fn match_pattern(&mut self, pat: Value, text: Value) -> Result<Value> {
        let pat = Pattern::extract(pat)?;
        let text = Str::extract(text)?;

        use std::collections::HashMap;

        struct Dict<'e> {
            inner: &'e mut Interpreter,
            locals: HashMap<usize, Str>,
            globals: HashMap<Ident, Option<Str>>,
        }

        impl<'e> Env for Dict<'e> {
            fn read_local(&mut self, name: usize) -> Str {
                if !self.locals.contains_key(&name) {
                    let value = self.inner.read::<Value>(name)
                        .unwrap().to_string();

                    let value = self.inner.strings.intern(&value).unwrap();
                    self.locals.insert(name, value);
                }

                self.locals.get(&name).unwrap().clone()
            }

            fn read_global(&mut self, name: &Ident) -> Option<Str> {
                if !self.globals.contains_key(name) {
                    let Dict { ref mut inner, ref mut globals, .. } = *self;

                    let dict = inner.globals.clone();

                    let value = dict.borrow().get(name).cloned().map(|val| {
                        inner.strings.intern(val.to_string()).unwrap()
                    });

                    globals.insert(name.clone(), value);
                }

                self.globals.get(name).unwrap().clone()
            }
        }

        let mut env = Dict {
            inner: self,
            locals: HashMap::new(),
            globals: HashMap::new(),
        };

        Ok(pat.matches(&mut env, text.as_ref()).into())
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
