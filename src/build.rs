use std::collections::{HashMap, HashSet};
use std::rc::Rc;

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
                self.ret();
            },

            Stmt::If { clauses, last } => {
                let after = self.gensym()?;

                let mut bodies = vec![];
                for (cond, body) in clauses.into_iter() {
                    let label = self.gensym()?;
                    self.tr_expr(cond)?;
                    self.jump_nonzero(label.clone());
                    bodies.push((label, body));
                }

                for stmt in last.into_iter() {
                    self.tr_stmt(stmt)?;
                }

                self.jump(after.clone());

                for (label, body) in bodies.into_iter() {
                    self.label(label)?;

                    for stmt in body.into_iter() {
                        self.tr_stmt(stmt)?;
                    }

                    self.jump(after.clone());
                }

                self.label(after)?;
            },

            Stmt::While { test, body } => {
                let before = self.gensym()?;
                let after = self.gensym()?;

                self.tr_expr(Expr::Not(test.clone().into()))?;
                self.jump_nonzero(after.clone());

                self.label(before.clone())?;
                for stmt in body.into_iter() {
                    self.tr_stmt(stmt)?;
                }

                self.tr_expr(test)?;
                self.jump_nonzero(before);

                self.label(after)?;
            },

            Stmt::Bare { rhs } => {
                self.tr_expr(rhs)?;
                self.discard();
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
                self.push_nil();
            },

            Expr::Literal(Literal::Int(int)) => {
                self.push_int(int);
            },

            Expr::Literal(Literal::Str(string)) => {
                self.push_str(&string);
            },

            Expr::Literal(Literal::List(items)) => {
                let len = items.len();

                for item in items.into_iter() {
                    self.tr_expr(item)?;
                }

                self.list(len);
            },

            Expr::Literal(Literal::Record(pairs)) => {
                self.rec();

                for (key, val) in pairs.into_iter() {
                    self.push_id(key);
                    self.tr_expr(val)?;
                    self.insert();
                }
            },

            Expr::Literal(Literal::Ident(id)) => {
                self.push_id(id);
            },

            Expr::Binop { lhs, op, rhs } => {
                self.tr_expr(*lhs)?;
                self.tr_expr(*rhs)?;
                self.binop(op);
            },

            Expr::Not(expr) => {
                self.tr_expr(*expr)?;
                self.not();
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

    pub fn jump(&mut self, dst: Ident) {
        self.program.code.push(Op::JUMP { dst });
    }

    pub fn jump_nonzero(&mut self, dst: Ident) {
        self.program.code.push(Op::JNZ { dst });
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

    pub fn push_nil(&mut self) {
        self.program.code.push(Op::NIL);
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

    pub fn binop(&mut self, op: ast::Binop) {
        let op = match op {
            ast::Binop::Add => Binop::ADD,
            ast::Binop::Sub => Binop::SUB,
            ast::Binop::Div => Binop::DIV,
            ast::Binop::Mul => Binop::MUL,
            ast::Binop::Idx => Binop::IDX,
        };

        self.program.code.push(Op::BINOP { op });
    }

    pub fn not(&mut self) {
        self.program.code.push(Op::NOT);
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

    asm.push_str("Hello, ");
    asm.push_str("world.");
    asm.call("str", 2).unwrap();
    asm.call("print", 1).unwrap();

    let mut w = eval::World::new(asm.build().unwrap());

    for _ in 0 .. 4 {
        w.step().unwrap();
    }
}
