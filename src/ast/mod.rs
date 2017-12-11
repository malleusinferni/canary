use super::*;
use value::*;
use opcode::*;

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ast/grammar.rs"));
}

pub use self::grammar::{parse_def, parse_statements, parse_module};

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

    Call {
        name: Ident,
        args: Vec<Expr>,
    },

    Return {
        rhs: Option<Expr>,
    },

    Nop,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Name(Ident),

    Literal(Literal),

    Binop {
        lhs: Box<Expr>,
        op: Binop,
        rhs: Box<Expr>,
    },
}

#[derive(Clone, Debug)]
pub struct Args(Vec<Ident>);

#[derive(Copy, Clone, Debug)]
pub enum Binop {
    Add,
    Sub,
    Div,
    Mul,
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

            Stmt::Call { name, args } => {
                let argc = args.len();

                for arg in args.into_iter() {
                    self.tr_expr(arg)?;
                }

                self.call(name.as_ref(), argc)?;
                self.discard();
            },

            Stmt::Assign { lhs, rhs } => {
                self.tr_expr(rhs)?;
                self.store(lhs)?;
            },

            Stmt::Return { rhs } => {
                self.tr_expr(rhs.unwrap_or(Expr::Literal(Literal::Nil)))?;
                self.ret();
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

                match op {
                    Binop::Add => self.add(),
                    Binop::Sub => self.sub(),
                    Binop::Div => self.div(),
                    Binop::Mul => self.mul(),
                }
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
            Stmt::Let { names: vec![x.clone()], },
            Stmt::Assign {
                lhs: x.clone(),
                rhs: Expr::Literal(Literal::Str("world".into())),
            },
            Stmt::Call {
                name: print.clone(),
                args: vec![Expr::Name(x.clone())],
            },
        },
    };

    translate(vec![src]).unwrap();
}

#[test]
fn syntax() {
    use ast::grammar::parse_def;
    use token::Tokenizer;

    let src = "sub foo() { x = y; }";
    let tokens = Tokenizer::new(src).collect::<Result<Vec<_>, _>>().unwrap();
    println!("Tokens: {:?}", tokens);

    parse_def(Tokenizer::new(src).spanned()).unwrap_or_else(|err| {
        println!("ERROR: {}", err);
        panic!("Aborting");
    });
}
