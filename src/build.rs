use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::*;
use ident::*;
use value::*;
use opcode::*;

pub struct Assembler {
    program: Program,
    strings: HashSet<Str>,
    scopes: Vec<HashMap<Ident, usize>>,
    next_gensym: usize,
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
            next_gensym: 0,
        }
    }

    pub fn build(mut self) -> Result<Program> {
        self.build_stdlib()?;
        Ok(self.program)
    }

    fn tr_stmt(&mut self, stmt: ast::Stmt) -> Result<()> {
        use ast::{Stmt, Expr, Literal};

        match stmt {
            Stmt::My { lhs, rhs } => {
                self.tr_expr(rhs.unwrap_or(Expr::Literal(Literal::Nil)))?;
                self.local(lhs)?;
            },

            Stmt::Assign { lhs, rhs } => {
                self.tr_expr(rhs)?;
                self.store(lhs)?;
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

                for stmt in last.into_iter() {
                    self.tr_stmt(stmt)?;
                }

                self.emit(Op::JUMP { dst: after.clone() });

                for (label, body) in bodies.into_iter() {
                    self.label(label)?;

                    for stmt in body.into_iter() {
                        self.tr_stmt(stmt)?;
                    }

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
                for stmt in body.into_iter() {
                    self.tr_stmt(stmt)?;
                }

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
        use ast::{Expr, Literal};

        match expr {
            Expr::Name(id) => {
                self.load(id)?;
            },

            Expr::Literal(Literal::Nil) => {
                self.emit(Op::NIL);
            },

            Expr::Literal(Literal::Int(int)) => {
                self.emit(Op::PUSHI { int });
            },

            Expr::Literal(Literal::Str(string)) => {
                let string = self.intern(&string);
                self.emit(Op::PUSHS { string });
            },

            Expr::Literal(Literal::List(items)) => {
                let len = items.len();

                for item in items.into_iter() {
                    self.tr_expr(item)?;
                }

                self.emit(Op::LIST { len });
            },

            Expr::Literal(Literal::Record(pairs)) => {
                self.emit(Op::REC);

                for (key, val) in pairs.into_iter() {
                    self.emit(Op::PUSHN { name: key });
                    self.tr_expr(val)?;
                    self.emit(Op::INS);
                }
            },

            Expr::Literal(Literal::Ident(id)) => {
                self.emit(Op::PUSHN { name: id });
            },

            Expr::Binop { lhs, op, rhs } => {
                self.tr_expr(*lhs)?;
                self.tr_expr(*rhs)?;
                self.binop(op);
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

    pub fn def(&mut self, def: ast::Def) -> Result<()> {
        let ast::Def { name, args, body } = def;

        let args = args.0;

        if self.scopes.is_empty() {
            let label = Func::Label(self.program.code.len());
            let argc = Argc::Exactly(args.len());
            self.program.functions.insert(name.clone(), (argc, label));

            let mut scope = HashMap::new();
            for (i, arg) in args.into_iter().enumerate() {
                scope.insert(arg, i);
            }
            self.scopes.push(scope);

            self.label(name)?;
            for stmt in body.into_iter() {
                self.tr_stmt(stmt)?;
            }

            // Implicit return
            // TODO: Allow any block to evaluate to an Expr
            self.tr_stmt(ast::Stmt::Return { rhs: None })?;

            self.undef()?;

            Ok(())
        } else {
            Err(Error::NonStaticFunction)
        }
    }

    fn undef(&mut self) -> Result<()> {
        if self.scopes.len() != 1 {
            Err(Error::InternalCompilerErr)
        } else {
            self.scopes.clear();
            Ok(())
        }
    }

    pub fn label(&mut self, id: Ident) -> Result<()> {
        if self.program.labels.contains_key(&id) {
            Err(Error::LabelRedefined)
        } else {
            let len = self.program.code.len();
            self.program.labels.insert(id, len);
            Ok(())
        }
    }

    fn emit(&mut self, op: Op<Ident>) {
        self.program.code.push(op);
    }

    pub fn gensym(&mut self) -> Result<Ident> {
        let id = Ident::new(format!("gensym_{}", self.next_gensym))?;
        self.next_gensym = self.next_gensym.checked_add(1)
            .ok_or(Error::InternalCompilerErr)?;
        Ok(id)
    }

    pub fn local(&mut self, id: Ident) -> Result<()> {
        let index = self.scopes.iter().map(|scope| scope.len()).sum();

        self.scopes.last_mut().ok_or(Error::InternalCompilerErr)
            .and_then(|scope| {
                if scope.contains_key(&id) {
                    Err(Error::VariableRenamed)
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

        Err(Error::VariableUndefined)
    }

    pub fn load(&mut self, id: Ident) -> Result<()> {
        let src = self.lookup(id)?;
        self.emit(Op::LOAD { src });
        Ok(())
    }

    pub fn store(&mut self, id: Ident) -> Result<()> {
        let dst = self.lookup(id)?;
        self.emit(Op::STORE { dst });
        Ok(())
    }

    pub fn call(&mut self, name: &str, argc: usize) -> Result<()> {
        let name = Ident::new(self.intern(name))?;
        self.emit(Op::CALL { name, argc });
        Ok(())
    }

    pub fn binop(&mut self, op: ast::Binop) {
        let op = match op {
            ast::Binop::Add => Binop::ADD,
            ast::Binop::Sub => Binop::SUB,
            ast::Binop::Div => Binop::DIV,
            ast::Binop::Mul => Binop::MUL,
            ast::Binop::Idx => Binop::IDX,
        };

        self.emit(Op::BINOP { op });
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
        let body = Func::Native(Arc::new(move |args| {
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

        self.def_native("assert", Exactly(1), |args| Ok({
            let arg = args.into_iter().next().unwrap();
            assert!(Int::extract(arg)? != 0);
        }))?;

        self.def_native("assert_eq", Exactly(2), |mut args| Ok({
            let rhs = args.pop().unwrap();
            let lhs = args.pop().unwrap();
            assert_eq!(lhs, rhs);
        }))?;

        Ok(())
    }
}

#[test]
fn hello() {
    let mut asm = Assembler::new();

    let string = asm.intern("Hello, ");
    asm.emit(Op::PUSHS { string });
    let string = asm.intern("world.");
    asm.emit(Op::PUSHS { string });
    asm.call("str", 2).unwrap();
    asm.call("print", 1).unwrap();

    let mut w = eval::World::new(asm.build().unwrap());

    for _ in 0 .. 4 {
        w.step().unwrap();
    }
}