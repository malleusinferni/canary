use std::iter::FromIterator;
use std::collections::BTreeMap;

use super::*;
use value::*;
use opcode::*;
use pattern::*;

use backpat::GroupNumber;

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
    groups: BTreeMap<GroupNumber, Str>,
    pc: usize,
}

impl Module {
    pub fn start(self) -> Result<Interpreter> {
        let mut this = Interpreter {
            frame: Frame {
                code: self.begin.clone(),
                locals: vec![],
                groups: BTreeMap::new(),
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
                let group = self.frame.groups.get(&num).cloned()
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
                let pat = self.compile_pattern(pat)?;
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

            Op::ASSERT { expr } => {
                if !(self.pop::<bool>()?) {
                    return Err(Error::Assert { expr });
                }
            },

            Op::MARK { len } => {
                if len > self.frame.locals.len() {
                    return Err(Error::MarkTooHigh);
                }

                self.frame.mark = len;
                self.frame.locals.drain(len ..);
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
                    groups: BTreeMap::new(),
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

    fn compile_pattern(&mut self, pat: pattern::Expr) -> Result<Pattern> {
        use std::collections::HashMap;

        use pattern::Var;

        let mut locals = HashMap::<usize, Str>::new();
        let mut globals = HashMap::<Ident, Str>::new();

        pat.map(|var| Ok(match *var {
            Var::Local { name } => {
                if !locals.contains_key(&name) {
                    let value = self.read::<Value>(name)?.to_string();
                    locals.insert(name, value.into());
                }

                locals.get(&name).cloned().unwrap()
            },

            Var::Global { ref name } => {
                if !globals.contains_key(name) {
                    let dict = self.globals.clone();
                    let value = dict.borrow().get(name).cloned();
                    if let Some(value) = value {
                        let value = value.to_string();
                        globals.insert(name.clone(), value.into());
                    }
                }

                globals.get(name).cloned()
                    .ok_or(Error::NoSuchGlobal)?
            },
        })).map(|pat| pat.into())
    }

    fn match_pattern(&mut self, pat: Value, text: Value) -> Result<Value> {
        let pat = Pattern::extract(pat)?;
        let text = Str::extract(text)?;

        let text = text.as_ref();

        let captures = pat.matches(text);

        let groups = &mut self.frame.groups;

        groups.clear();

        Ok(captures.map(|captures| {
            for (id, start, end) in captures.into_iter() {
                let text = Str::from(&text[start .. end]);
                groups.insert(id, text);
            }

            true
        }).unwrap_or({
            false
        }).into())
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
