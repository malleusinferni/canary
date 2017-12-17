use super::*;
use value::*;

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ast/grammar.rs"));
}

pub use self::grammar::{parse_def, parse_block_body, parse_module};

#[derive(Clone, Debug)]
pub struct Module {
    pub begin: Vec<Stmt>,
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
        lhs: Expr,
        rhs: Expr,
    },

    Return {
        rhs: Option<Expr>,
    },

    Assert {
        rhs: Expr,
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
    Parens(Box<Expr>),

    Local(Ident),

    Global(Ident),

    Group(u8),

    Call {
        name: Ident,
        args: Vec<Expr>,
    },

    Literal(Literal),

    Str(Vec<Expr>),

    List(Vec<Expr>),

    Record(Vec<(Ident, Expr)>),

    Binop {
        lhs: Box<Expr>,
        op: Binop,
        rhs: Box<Expr>,
    },

    And {
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    },

    Or {
        lhs: Box<Expr>,
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
    Match,
    Equal,
    NotEqual,
}

#[derive(Clone, Debug)]
pub enum Literal {
    Int(Int),
    Str(Str),
    Ident(Ident),
    Pattern(pattern::Ast),
    Nil,
}

impl Binop {
    pub fn apply(self, lhs: Expr, rhs: Expr) -> Expr {
        Expr::Binop { lhs: Box::new(lhs), op: self, rhs: Box::new(rhs) }
    }
}

mod display {
    use std::fmt::{Display, Formatter, Result};

    use super::*;

    fn uncomma<T: Display>(items: &[T]) -> String {
        items.iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }

    impl Display for Expr {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Expr::Parens(ref expr) => {
                    write!(f, "({})", expr)
                },

                Expr::Literal(ref lit) => {
                    write!(f, "{}", lit)
                },

                Expr::And { ref lhs, ref rhs } => {
                    write!(f, "{} and {}", lhs, rhs)
                },

                Expr::Or { ref lhs, ref rhs } => {
                    write!(f, "{} or {}", lhs, rhs)
                },

                Expr::Not(ref rhs) => {
                    write!(f, "not {}", rhs)
                },

                Expr::Binop { ref lhs, op, ref rhs } => match op {
                    Binop::Add => write!(f, "{} + {}", lhs, rhs),
                    Binop::Sub => write!(f, "{} - {}", lhs, rhs),
                    Binop::Div => write!(f, "{} / {}", lhs, rhs),
                    Binop::Mul => write!(f, "{} * {}", lhs, rhs),
                    Binop::Idx => write!(f, "{}[{}]", lhs, rhs),
                    Binop::Match => write!(f, "{} =~ {}", lhs, rhs),
                    Binop::Equal => write!(f, "{} eq {}", lhs, rhs),
                    Binop::NotEqual => write!(f, "{} ne {}", lhs, rhs),
                },

                Expr::Local(ref id) => {
                    write!(f, "${}", id)
                },

                Expr::Global(ref id) => {
                    write!(f, "%{}", id)
                },

                Expr::Str(ref _items) => {
                    // FIXME
                    write!(f, "{{interpolated string}}")
                },

                Expr::Group(num) => {
                    write!(f, "${}", num)
                },

                Expr::List(ref items) => {
                    write!(f, "[{}]", uncomma(items))
                },

                Expr::Record(_) => {
                    write!(f, "{{record}}")
                },

                Expr::Call { ref name, ref args } => {
                    write!(f, "{}({})", name, uncomma(args))
                },
            }
        }
    }

    impl Display for Literal {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Literal::Nil => write!(f, "()"),
                Literal::Int(i) => write!(f, "{}", i),
                Literal::Str(ref s) => write!(f, "{:?}", s),
                Literal::Ident(ref n) => write!(f, ":{}", n),
                Literal::Pattern(ref p) => p.fmt(f),
            }
        }
    }
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
                lhs: Expr::Local(x.clone()),
                rhs: Expr::Literal(Literal::Str(world)),
            },
            Stmt::Bare {
                rhs: Expr::Call {
                    name: print,
                    args: vec![Expr::Local(x.clone())],
                },
            },
        },
    };

    let module = Module {
        begin: vec![],
        defs: vec![src],
    };

    module.translate().unwrap();
}

#[test]
fn syntax() {
    use ast::grammar::parse_def;
    use token::Tokenizer;

    let src = &[
        "sub assign() { $x = $y; }",
        "sub simple_if() { if 0 { } }",
        "sub if_else() { if 1 { 1; } else if 2 { 2; } else { 3; } }",
        "sub while_loop() { while 1 { } }",
        "sub globals() { %X = %Y; }",
        "sub symbols() { my $a = :b; :c + :d; }",
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
