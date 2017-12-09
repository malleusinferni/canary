use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use super::*;
use ident::*;
use value::*;

pub struct Program {
    code: Vec<Op>,
    labels: HashMap<Ident, usize>,
    functions: HashMap<Ident, (Argc, Func)>,
}

pub struct Assembler {
    program: Program,
    strings: HashSet<Str>,
    scopes: Vec<HashMap<Ident, usize>>,
}

pub type NativeFn = Rc<Fn(Vec<Value>) -> Result<Value>>;

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
pub enum Op {
    RET,
    DROP,
    NOT,
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
    JUMP { dst: Ident, },
    JNZ { dst: Ident, },
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
    pub fn fetch(&self, pc: usize) -> Result<Op> {
        self.code.get(pc).cloned().ok_or(Error::Okay)
    }

    pub fn jump(&self, label: Ident) -> Result<usize> {
        self.labels.get(&label).cloned().ok_or(Error::Okay)
    }

    pub fn call(&self, name: Ident, argv: &[Value]) -> Result<Func> {
        let (wanted, func) = self.functions.get(&name).cloned()
            .ok_or(Error::Okay)?;

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

impl Assembler {
    pub fn new() -> Self {
        Assembler {
            program: Program {
                code: vec![],
                labels: HashMap::new(),
                functions: HashMap::new(),
            },

            strings: HashSet::new(),
            scopes: vec![],
        }
    }

    pub fn build(mut self) -> Result<Program> {
        self.build_stdlib()?;
        Ok(self.program)
    }

    pub fn def(&mut self, id: Ident, args: Vec<Ident>) -> Result<()> {
        if self.scopes.is_empty() {
            let label = Func::Label(self.program.code.len());
            let argc = Argc::Exactly(args.len());
            self.program.functions.insert(id, (argc, label));

            let mut scope = HashMap::new();
            for (i, arg) in args.into_iter().enumerate() {
                scope.insert(arg, i);
            }
            self.scopes.push(scope);

            Ok(())
        } else {
            Err(Error::Okay)
        }
    }

    pub fn undef(&mut self) -> Result<()> {
        if self.scopes.len() != 1 {
            Err(Error::Okay)
        } else {
            self.scopes.clear();
            Ok(())
        }
    }

    pub fn label(&mut self, id: Ident) -> Result<()> {
        if self.program.labels.contains_key(&id) {
            Err(Error::Okay)
        } else {
            let len = self.program.code.len();
            self.program.labels.insert(id, len);
            Ok(())
        }
    }

    pub fn local(&mut self, id: Ident) -> Result<()> {
        let index = self.scopes.iter().map(|scope| scope.len()).sum();

        self.scopes.last_mut().ok_or(Error::Okay).and_then(|scope| {
            if scope.contains_key(&id) {
                Err(Error::Okay)
            } else {
                Ok({ scope.insert(id, index);  })
            }
        })
    }

    fn lookup(&self, id: Ident) -> Result<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&index) = scope.get(&id) {
                return Ok(index);
            }
        }

        Err(Error::Okay)
    }

    pub fn load(&mut self, id: Ident) -> Result<()> {
        let src = self.lookup(id)?;
        self.program.code.push(Op::LOAD { src });
        Ok(())
    }

    pub fn store(&mut self, id: Ident) -> Result<()> {
        let dst = self.lookup(id)?;
        self.program.code.push(Op::STORE { dst });
        Ok(())
    }

    pub fn push_str(&mut self, s: &str) {
        let string = self.intern(s);
        self.program.code.push(Op::PUSHS { string });
    }

    pub fn push_int(&mut self, int: i32) {
        self.program.code.push(Op::PUSHI { int })
    }

    pub fn push_id(&mut self, name: Ident) {
        self.program.code.push(Op::PUSHN { name })
    }

    pub fn discard(&mut self) {
        self.program.code.push(Op::DROP);
    }

    pub fn ret(&mut self) {
        self.program.code.push(Op::RET);
    }

    pub fn call(&mut self, name: &str, argc: usize) -> Result<()> {
        let name = Ident::new(self.intern(name))?;
        self.program.code.push(Op::CALL { name, argc });
        Ok(())
    }

    pub fn add(&mut self) {
        self.program.code.push(Op::BINOP { op: Binop::ADD });
    }

    pub fn sub(&mut self) {
        self.program.code.push(Op::BINOP { op: Binop::SUB });
    }

    pub fn div(&mut self) {
        self.program.code.push(Op::BINOP { op: Binop::DIV });
    }

    pub fn mul(&mut self) {
        self.program.code.push(Op::BINOP { op: Binop::MUL });
    }

    pub fn list(&mut self, len: usize) {
        self.program.code.push(Op::LIST { len });
    }

    pub fn rec(&mut self) {
        self.program.code.push(Op::REC);
    }

    pub fn insert(&mut self) {
        self.program.code.push(Op::INS);
    }

    fn intern(&mut self, s: &str) -> Str {
        if !self.strings.contains(s) {
            self.strings.insert(s.into());
        }

        self.strings.get(s).cloned().unwrap()
    }

    pub fn def_native<F, V>(&mut self, name: &str, argc: Argc, body: F)
        -> Result<()>
        where F: 'static + Fn(Vec<Value>) -> Result<V>,
              V: Into<Value>
    {
        let name = Ident::new(self.intern(name))?;
        let body = Func::Native(Rc::new(move |args| {
            let result = body(args)?;
            Ok(result.into())
        }));

        self.program.functions.insert(name, (argc, body));

        Ok(())
    }

    fn build_stdlib(&mut self) -> Result<()> {
        use self::Argc::*;

        fn map_to_string(items: Vec<Value>) -> Vec<String> {
            items.into_iter().map(|i| format!("{}", i)).collect()
        }

        self.def_native("print", AtLeast(1), |args| Ok({
            println!("{}", map_to_string(args).join(" "));
        }))?;

        self.def_native("str", AtLeast(1), |args| Ok({
            Str::from(map_to_string(args).concat())
        }))?;

        Ok(())
    }
}
