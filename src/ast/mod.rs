use super::*;
use value::*;

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ast/grammar.rs"));
}

pub use self::grammar::{parse_def, parse_block_body, parse_module};

#[derive(Clone, Debug)]
pub struct Module {
    pub defs: Vec<Def>,
}

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

    List(Vec<Expr>),

    Record(Vec<(Ident, Expr)>),

    Binop {
        lhs: Box<Expr>,
        op: Binop,
        rhs: Box<Expr>,
    },

    Not(Box<Expr>),
}

#[derive(Clone, Debug)]
pub struct Args(pub Vec<Ident>);

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
    Ident(Ident),
    Nil,
}

#[test]
fn translation() {
    let mut strings = Strings::new();
    let x: Ident = strings.intern("x").unwrap();
    let hello = strings.intern("hello").unwrap();
    let world = strings.intern("world").unwrap();
    let print = strings.intern("print").unwrap();

    let src = Def {
        name: hello,
        args: Args(vec![]),
        body: vec!{
            Stmt::My { lhs: x.clone(), rhs: None, },
            Stmt::Assign {
                lhs: x.clone(),
                rhs: Expr::Literal(Literal::Str(world)),
            },
            Stmt::Bare {
                rhs: Expr::Call {
                    name: print,
                    args: vec![Expr::Name(x.clone())],
                },
            },
        },
    };

    let module = Module {
        defs: vec![src],
    };

    module.translate().unwrap();
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
