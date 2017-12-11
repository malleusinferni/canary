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

impl Program {
    pub fn def(&mut self, def: ast::Def) -> Result<()> {
        use ast::Def;

        let Def { name, args, body } = def;
        let args = args.0;
        let argc = Argc::Exactly(args.len());

        let mut asm = Assembler::new(&mut self.strings, args);

        for stmt in body.into_iter() {
            asm.tr_stmt(stmt)?;
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

        let mut std = Program {
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

        std.def_native("assert", Exactly(1), |args| Ok({
            let arg = args.into_iter().next().unwrap();
            assert!(Int::extract(arg)? != 0);
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
    pub fn translate(self) -> Result<Program> {
        let mut module = Program::stdlib()?;

        for def in self.defs.into_iter() {
            module.def(def)?;
        }

        Ok(module)
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
            Op::DROP => Op::DROP,
            Op::GLOBALS => Op::GLOBALS,
            Op::INS => Op::INS,
            Op::LOAD { src } => Op::LOAD { src },
            Op::STORE { dst } => Op::STORE { dst },
            Op::PUSHI { int } => Op::PUSHI { int },
            Op::PUSHS { string } => Op::PUSHS { string },
            Op::PUSHN { name } => Op::PUSHN { name },
            Op::LIST { len } => Op::LIST { len },
            Op::REC => Op::REC,
            Op::CALL { name, argc } => Op::CALL { name, argc },
            Op::BINOP { op } => Op::BINOP { op },
        })).collect::<Result<Vec<Op>>>()?;

        Ok(InterpretedFn::from_vec(code))
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
        use ast::Expr;

        match expr {
            Expr::Name(id) => {
                self.load(id)?;
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

    fn local(&mut self, id: Ident) -> Result<()> {
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
        };

        self.emit(Op::BINOP { op });
    }
}
