use super::*;
use value::*;
use opcode::*;

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ast/grammar.rs"));
}

pub use self::grammar::{parse_def, parse_block_body, parse_module};

#[derive(Clone, Debug)]
pub struct Def {
    pub name: Ident,
    pub args: Args,
    pub body: Vec<Stmt>,
}

#[derive(Clone, Debug)]
pub enum Stmt {
    My {
        lhs: Ident,
        rhs: Option<Expr>,
    },

    Assign {
        lhs: Ident,
        rhs: Expr,
    },

    Return {
        rhs: Option<Expr>,
    },

    If {
        clauses: Vec<(Expr, Vec<Stmt>)>,
        last: Vec<Stmt>,
    },

    While {
        test: Expr,
        body: Vec<Stmt>,
    },

    Bare {
        rhs: Expr,
    },

    Nop,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Name(Ident),

    Call {
        name: Ident,
        args: Vec<Expr>,
    },

    Literal(Literal),

    Binop {
        lhs: Box<Expr>,
        op: Binop,
        rhs: Box<Expr>,
    },

    Not(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Args(Vec<Ident>);

#[derive(Copy, Clone, Debug)]
pub enum Binop {
    Add,
    Sub,
    Div,
    Mul,
    Idx,
}

#[derive(Clone, Debug)]
pub enum Literal {
    Int(Int),
    Str(Str),
    List(Vec<Expr>),
    Record(Vec<(Ident, Expr)>),
    Ident(Ident),
    Nil,
}

impl Assembler {
    fn tr_stmt(&mut self, stmt: Stmt) -> Result<()> {
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

    fn tr_expr(&mut self, expr: Expr) -> Result<()> {
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
}

pub fn translate(ast: Vec<Def>) -> Result<Program> {
    let mut asm = Assembler::new();

    for Def { name, args, body } in ast.into_iter() {
        asm.def(name.clone(), args.0)?;
        asm.label(name)?;

        for stmt in body.into_iter() {
            asm.tr_stmt(stmt)?;
        }

        // Implicit return nil at end of function
        asm.tr_stmt(Stmt::Return { rhs: None })?;
        asm.undef()?;
    }

    Ok(asm.build()?)
}

#[test]
fn translation() {
    let x = Ident::new("x").unwrap();
    let hello = Ident::new("hello").unwrap();
    let print = Ident::new("print").unwrap();

    let src = Def {
        name: hello.clone(),
        args: Args(vec![]),
        body: vec!{
            Stmt::My { lhs: x.clone(), rhs: None, },
            Stmt::Assign {
                lhs: x.clone(),
                rhs: Expr::Literal(Literal::Str("world".into())),
            },
            Stmt::Bare {
                rhs: Expr::Call {
                    name: print.clone(),
                    args: vec![Expr::Name(x.clone())],
                },
            },
        },
    };

    translate(vec![src]).unwrap();
}

#[test]
fn syntax() {
    use ast::grammar::parse_def;
    use token::Tokenizer;

    let src = &[
        "sub assign() { x = y; }",
        "sub simple_if() { if 0 { } }",
        "sub if_else() { if 1 { 1; } else if 2 { 2; } else { 3; } }",
        "sub while_loop() { while 1 { } }",
    ];

    for src in src {
        parse_def(Tokenizer::new(src).spanned()).unwrap_or_else(|err| {
            println!("ERROR: {}", err);
            let tokens: Vec<_> = Tokenizer::new(src)
                .collect::<Result<_, _>>()
                .unwrap();

            println!("Tokens: {:?}", tokens);

            panic!("Test failed");
        });
    }
}
