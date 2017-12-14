use std::collections::HashMap;
use std::sync::Arc;

use super::*;
use ident::*;
use value::*;
use opcode::*;

pub struct Assembler<'a> {
    code: Vec<Op<Sym>>,
    strings: &'a mut Strings,
    labels: HashMap<Sym, usize>,
    scopes: Vec<HashMap<Ident, usize>>,
    next_gensym: usize,
}

#[derive(Copy, Clone, Eq, Hash, PartialEq)]
struct Sym(usize);

impl Module {
    pub fn def(&mut self, def: ast::Def) -> Result<()> {
        use ast::Def;

        let Def { name, args, body } = def;
        let args = args.0;
        let argc = Argc::Exactly(args.len());

        let mut asm = Assembler::new(&mut self.strings, args);

        for stmt in body.into_iter() {
            asm.tr_stmt(stmt).map_err(|cause| {
                Error::WithContext {
                    cause: cause.into(),
                    context: format!("sub {}", &name),
                }
            })?;
        }

        // Implicit return
        // TODO: Allow any block to evaluate to an Expr
        asm.tr_stmt(ast::Stmt::Return { rhs: None })?;

        let func = Func::Interpreted(asm.build()?);

        self.functions.insert(name.clone(), (argc, func));

        Ok(())
    }

    pub fn def_native<F, V>(&mut self, name: &str, argc: Argc, body: F)
        -> Result<()>
        where F: 'static + Fn(Vec<Value>) -> Result<V>,
              V: Into<Value>
    {
        let name = self.strings.intern(name)?;
        let body = Func::Native(Arc::new(move |args| {
            let result = body(args)?;
            Ok(result.into())
        }));

        self.functions.insert(name, (argc, body));

        Ok(())
    }

    pub fn stdlib() -> Result<Self> {
        use self::Argc::*;

        let mut std = Module {
            begin: InterpretedFn::from_vec(vec![]),
            strings: Strings::new(),
            functions: HashMap::new(),
        };

        fn map_to_string(items: Vec<Value>) -> Vec<String> {
            items.into_iter().map(|i| format!("{}", i)).collect()
        }

        std.def_native("print", AtLeast(1), |args| Ok({
            println!("{}", map_to_string(args).join(" "));
        }))?;

        std.def_native("str", AtLeast(1), |args| Ok({
            Str::from(map_to_string(args).concat())
        }))?;

        std.def_native("len", Exactly(1), |mut args| Ok({
            let arg = List::extract(args.pop().unwrap())?;
            let arg = arg.borrow();
            arg.len() as Int
        }))?;

        std.def_native("split", AtLeast(1), |_args| {
            //use pattern::*;

            //let mut args = args.into_iter();
            //let text = Str::extract(args.next().unwrap())?;
            //let pat = match args.next() {
            //    Some(pat) => Pattern::extract(pat)?,
            //    None => Pattern::Find(" ".into())
            //};

            //match pat {
            //    Pattern::Find(pat) => {
            //        let pat: &str = pat.as_ref();
            //        Value::from_iter(text.split(pat).map(|s| {
            //            Str::from(s)
            //        }))
            //    }
            //}

            if false { return Ok(()) }

            return Err(Error::UnimplementedFeature {
                feature: "pattern matching",
            });
        })?;

        std.def_native("new", AtLeast(0), |args| Ok({
            if !args.is_empty() {
                println!("Warning: Arguments to new() not implemented");
            }

            Record::new(HashMap::new().into())
        }))?;

        std.def_native("assert", Exactly(1), |args| Ok({
            let arg = args.into_iter().next().unwrap();
            assert!(bool::extract(arg)?);
        }))?;

        std.def_native("assert_eq", Exactly(2), |mut args| Ok({
            let rhs = args.pop().unwrap();
            let lhs = args.pop().unwrap();
            assert_eq!(lhs, rhs);
        }))?;

        Ok(std)
    }
}

impl ast::Module {
    pub fn translate(self) -> Result<Module> {
        let mut module = Module::stdlib()?;

        module.begin = {
            let mut asm = Assembler::new(&mut module.strings, vec![]);

            for stmt in self.begin.into_iter() {
                asm.tr_stmt(stmt)?;
            }

            asm.build()?
        };

        for def in self.defs.into_iter() {
            module.def(def)?;
        }

        Ok(module)
    }
}

enum Lvalue {
    Store { lhs: Ident },
    Insert { lhs: ast::Expr, idx: ast::Expr },
    SetGlobal { name: Ident },
}

impl ast::Expr {
    fn as_lvalue(self) -> Result<Lvalue> {
        use ast::{Expr, Binop};

        match self {
            Expr::Local(lhs) => Ok(Lvalue::Store { lhs }),

            Expr::Global(name) => Ok(Lvalue::SetGlobal { name }),

            Expr::Binop { lhs, rhs, op: Binop::Idx } => {
                Ok(Lvalue::Insert { lhs: *lhs, idx: *rhs })
            },

            _ => Err(Error::IllegalLvalue),
        }
    }
}

impl<'a> Assembler<'a> {
    fn new(strings: &'a mut Strings, args: Vec<Ident>) -> Self {
        let mut scope = HashMap::new();
        for (i, arg) in args.into_iter().enumerate() {
            scope.insert(arg, i);
        }

        Assembler {
            strings,
            code: vec![],
            scopes: vec![scope],
            labels: HashMap::new(),
            next_gensym: 0,
        }
    }

    fn build(self) -> Result<InterpretedFn> {
        let Assembler { code, labels, .. } = self;

        let resolve = |label| -> Result<usize> {
            labels.get(&label).cloned().ok_or(Error::NoSuchLabel)
        };

        let code = code.into_iter().map(|op| Ok(match op {
            Op::JUMP { dst } => {
                let dst = resolve(dst)?;
                Op::JUMP { dst }
            },

            Op::JNZ { dst } => {
                let dst = resolve(dst)?;
                Op::JNZ { dst }
            },

            Op::NIL => Op::NIL,
            Op::RET => Op::RET,
            Op::NOT => Op::NOT,
            Op::DUP => Op::DUP,
            Op::DROP => Op::DROP,
            Op::GLOBALS => Op::GLOBALS,
            Op::INS => Op::INS,
            Op::LOAD { src } => Op::LOAD { src },
            Op::STORE { dst } => Op::STORE { dst },
            Op::GROUP { num } => Op::GROUP { num },
            Op::PUSHI { int } => Op::PUSHI { int },
            Op::PUSHS { string } => Op::PUSHS { string },
            Op::PUSHN { name } => Op::PUSHN { name },
            Op::PAT { pat } => Op::PAT { pat },
            Op::LIST { len } => Op::LIST { len },
            Op::STR { len } => Op::STR { len },
            Op::REC => Op::REC,
            Op::CALL { name, argc } => Op::CALL { name, argc },
            Op::BINOP { op } => Op::BINOP { op },
            Op::MARK { len } => Op::MARK { len },
        })).collect::<Result<Vec<Op>>>()?;

        Ok(InterpretedFn::from_vec(code))
    }

    fn enter(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn leave(&mut self) -> Result<()> {
        Ok({ self.scopes.pop().ok_or(Error::InternalCompilerErr)?; })
    }

    fn tr_block(&mut self, body: Vec<ast::Stmt>) -> Result<()> {
        let len = self.depth();
        self.enter();
        for stmt in body.into_iter() {
            self.tr_stmt(stmt)?;
        }
        self.leave()?;
        self.emit(Op::MARK { len });
        Ok(())
    }

    fn tr_stmt(&mut self, stmt: ast::Stmt) -> Result<()> {
        use ast::{Stmt, Expr, Literal};

        match stmt {
            Stmt::My { lhs, rhs } => {
                self.tr_expr(rhs.unwrap_or(Expr::Literal(Literal::Nil)))?;
                self.local(lhs)?;
            },

            Stmt::Assign { lhs, rhs } => match lhs.as_lvalue()? {
                Lvalue::Store { lhs } => {
                    self.tr_expr(rhs)?;
                    self.store(lhs)?;
                },

                Lvalue::Insert { lhs, idx } => {
                    self.tr_expr(rhs)?;
                    self.tr_expr(idx)?;
                    self.tr_expr(lhs)?;
                    self.emit(Op::INS);
                },

                Lvalue::SetGlobal { name } => {
                    self.tr_expr(rhs)?;
                    self.emit(Op::PUSHN { name });
                    self.emit(Op::GLOBALS);
                    self.emit(Op::INS);
                },
            },

            Stmt::Return { rhs } => {
                self.tr_expr(rhs.unwrap_or(Expr::Literal(Literal::Nil)))?;
                self.emit(Op::RET);
            },

            Stmt::If { clauses, last } => {
                let after = self.gensym()?;

                let mut bodies = vec![];
                for (cond, body) in clauses.into_iter() {
                    let label = self.gensym()?;
                    self.tr_expr(cond)?;
                    self.emit(Op::JNZ { dst: label.clone() });
                    bodies.push((label, body));
                }

                self.tr_block(last)?;
                self.emit(Op::JUMP { dst: after.clone() });

                for (label, body) in bodies.into_iter() {
                    self.label(label)?;
                    self.tr_block(body)?;
                    self.emit(Op::JUMP { dst: after.clone() });
                }

                self.label(after)?;
            },

            Stmt::While { test, body } => {
                let before = self.gensym()?;
                let after = self.gensym()?;

                self.tr_expr(Expr::Not(test.clone().into()))?;
                self.emit(Op::JNZ { dst: after.clone() });

                self.label(before.clone())?;
                self.tr_block(body)?;

                self.tr_expr(test)?;
                self.emit(Op::JNZ { dst: before.clone() });

                self.label(after)?;
            },

            Stmt::Bare { rhs } => {
                self.tr_expr(rhs)?;
                self.emit(Op::DROP);
            },

            Stmt::Nop => {
                // Do nothing
            },
        }

        Ok(())
    }

    fn tr_expr(&mut self, expr: ast::Expr) -> Result<()> {
        use ast::Expr;

        match expr {
            Expr::Local(id) => {
                self.load(id)?;
            },

            Expr::Global(id) => {
                self.emit(Op::GLOBALS);
                self.emit(Op::PUSHN { name: id });
                self.emit(Op::BINOP { op: Binop::IDX });
            },

            Expr::Group(num) => {
                self.emit(Op::GROUP { num });
            },

            Expr::Literal(lit) => {
                self.push(lit)?;
            },

            Expr::List(items) => {
                let len = items.len();

                for item in items.into_iter() {
                    self.tr_expr(item)?;
                }

                self.emit(Op::LIST { len });
            },

            Expr::Str(items) => {
                let len = items.len();
                for item in items.into_iter() {
                    self.tr_expr(item)?;
                }

                self.emit(Op::STR { len });
            },

            Expr::Record(pairs) => {
                self.emit(Op::REC);

                for (key, val) in pairs.into_iter() {
                    self.emit(Op::PUSHN { name: key });
                    self.tr_expr(val)?;
                    self.emit(Op::INS);
                }
            },

            Expr::Binop { lhs, op, rhs } => {
                self.tr_expr(*lhs)?;
                self.tr_expr(*rhs)?;
                self.binop(op);
            },

            Expr::And { lhs, rhs } => {
                let after = self.gensym()?;

                // lhs ? rhs : lhs

                self.tr_expr(*lhs)?;
                self.emit(Op::DUP);
                self.emit(Op::NOT);
                self.emit(Op::JNZ { dst: after });
                self.emit(Op::DROP);
                self.tr_expr(*rhs)?;
                self.label(after)?;
            },

            Expr::Or { lhs, rhs } => {
                let after = self.gensym()?;

                // lhs ? lhs : rhs

                self.tr_expr(*lhs)?;
                self.emit(Op::DUP);
                self.emit(Op::JNZ { dst: after });
                self.emit(Op::DROP);
                self.tr_expr(*rhs)?;
                self.label(after)?;
            },

            Expr::Not(expr) => {
                self.tr_expr(*expr)?;
                self.emit(Op::NOT);
            },

            Expr::Call { name, args } => {
                let argc = args.len();

                for arg in args.into_iter() {
                    self.tr_expr(arg)?;
                }

                self.call(name.as_ref(), argc)?;
            },
        }

        Ok(())
    }

    fn push<S: Into<ast::Literal>>(&mut self, lit: S) -> Result<()> {
        use ast::Literal;

        match lit.into() {
            Literal::Int(int) => {
                self.emit(Op::PUSHI { int });
            },

            Literal::Str(string) => {
                let string = self.strings.intern(&string)?;
                self.emit(Op::PUSHS { string });
            },

            Literal::Pattern(pat) => {
                self.emit(Op::PAT { pat });
            },

            Literal::Ident(id) => {
                self.emit(Op::PUSHN { name: id });
            },

            Literal::Nil => {
                self.emit(Op::NIL);
            },
        }

        Ok(())
    }

    fn label(&mut self, label: Sym) -> Result<()> {
        if self.labels.contains_key(&label) {
            Err(Error::LabelRedefined)
        } else {
            let len = self.code.len();
            self.labels.insert(label, len);
            Ok(())
        }
    }

    fn emit(&mut self, op: Op<Sym>) {
        self.code.push(op);
    }

    fn gensym(&mut self) -> Result<Sym> {
        let sym = Sym(self.next_gensym);
        self.next_gensym = self.next_gensym.checked_add(1)
            .ok_or(Error::InternalCompilerErr)?;
        Ok(sym)
    }

    fn depth(&self) -> usize {
        self.scopes.iter().map(|scope| scope.len()).sum()
    }

    fn local(&mut self, id: Ident) -> Result<()> {
        let index = self.depth();

        if let Some(scope) = self.scopes.last_mut() {
            if scope.contains_key(&id) {
                return Err(Error::VariableRenamed);
            }

            scope.insert(id, index);
        } else {
            return Err(Error::InternalCompilerErr);
        }

        self.emit(Op::MARK { len: index + 1 });

        Ok(())
    }

    fn lookup(&self, id: Ident) -> Result<usize> {
        for scope in self.scopes.iter().rev() {
            if let Some(&index) = scope.get(&id) {
                return Ok(index);
            }
        }

        Err(Error::VariableUndefined)
    }

    fn load(&mut self, id: Ident) -> Result<()> {
        let src = self.lookup(id)?;
        self.emit(Op::LOAD { src });
        Ok(())
    }

    fn store(&mut self, id: Ident) -> Result<()> {
        let dst = self.lookup(id)?;
        self.emit(Op::STORE { dst });
        Ok(())
    }

    fn call(&mut self, name: &str, argc: usize) -> Result<()> {
        let name = self.strings.intern(name)?;
        self.emit(Op::CALL { name, argc });
        Ok(())
    }

    fn binop(&mut self, op: ast::Binop) {
        let op = match op {
            ast::Binop::Add => Binop::ADD,
            ast::Binop::Sub => Binop::SUB,
            ast::Binop::Div => Binop::DIV,
            ast::Binop::Mul => Binop::MUL,
            ast::Binop::Idx => Binop::IDX,
            ast::Binop::Match => Binop::MATCH,
            ast::Binop::Equal => Binop::EQ,
            ast::Binop::NotEqual => Binop::NE,
        };

        self.emit(Op::BINOP { op });
    }
}
