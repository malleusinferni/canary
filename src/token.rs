use super::*;
use ident::*;
use value::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    ID(Ident),
    INT(Int),
    STR(Str),
    LPAR,
    RPAR,
    LSQB,
    RSQB,
    LCBR,
    RCBR,
    DEF,
    LET,
    COLON,
    RETURN,
    EQUAL,
    COMMA,
    ADD,
    SUB,
    DIV,
    MUL,
    EOL,
    BEGIN,
    END,
}

use std::str::Chars;
use std::iter::Peekable;

pub struct Spanned<T: Iterator<Item=Result<Token>>> {
    inner: T,
}

impl<T: Iterator<Item=Result<Token>>> Iterator for Spanned<T> {
    type Item = Result<(usize, Token, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|r| r.map(|t| (0, t, 0)))
    }
}

pub struct Tokenizer<'a> {
    input: Peekable<Chars<'a>>,
    buf: Vec<Token>,
    bol: bool,
    indents: Vec<usize>,
}

impl<'a> Tokenizer<'a> {
    pub fn new(src: &'a str) -> Self {
        Tokenizer {
            input: src.chars().peekable(),
            buf: vec![],
            bol: true,
            indents: vec![],
        }
    }

    pub fn spanned(self) -> Spanned<Self> {
        Spanned { inner: self }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.input.peek().is_none() {
            while let Some(_) = self.indents.pop() {
                self.buf.push(Token::END);
            }
        }

        if let Some(token) = self.buf.pop() {
            return Some(Ok(token));
        }

        while let Some(&s) = self.input.peek() {
            if s == '#' {
                while let Some(n) = self.input.next() {
                    if n == '\n' {
                        self.bol = true;
                        return Some(Ok(Token::EOL));
                    }
                }
            } else if s == '\n' {
                self.bol = true;
                self.input.next();
                return Some(Ok(Token::EOL));
            } else if s.is_whitespace() {
                let _ = self.input.next();
                if !self.bol { continue; }

                // Handle indentation
                self.bol = false;
                let mut width = if s == '\t' { 8 } else { 1 };

                while let Some(&n) = self.input.peek() {
                    match n {
                        '\t' => width += 8,
                        ' ' => width += 1,
                        _ => break,
                    }

                    let _ = self.input.next();
                }

                loop {
                    let &prev = self.indents.last().unwrap_or(&0);

                    if prev == width {
                        break;
                    }

                    if prev > width {
                        self.indents.pop();
                        self.buf.push(Token::END);
                    } else if prev < width {
                        self.indents.push(width);
                        return Some(Ok(Token::BEGIN));
                    }
                }

                if let Some(token) = self.buf.pop() {
                    return Some(Ok(token));
                }
            } else {
                break;
            }
        }

        if let Some(&s) = self.input.peek() {
            if self.bol && !s.is_whitespace() {
                while let Some(_) = self.indents.pop() {
                    if self.indents.is_empty() {
                        return Some(Ok(Token::END));
                    } else {
                        self.buf.push(Token::END);
                    }
                }
            }
        }

        self.bol = false;

        let first = self.input.next()?;

        Some(Ok(match first {
            '(' => Token::LPAR,
            ')' => Token::RPAR,
            '[' => Token::LSQB,
            ']' => Token::RSQB,
            '{' => Token::LCBR,
            '}' => Token::RCBR,

            ':' => Token::COLON,
            '=' => Token::EQUAL,
            ',' => Token::COMMA,
            ';' => Token::EOL,

            '+' => Token::ADD,
            '-' => Token::SUB,
            '/' => Token::DIV,
            '*' => Token::MUL,

            '"' => {
                let mut buf = String::new();
                while let Some(c) = self.input.next() {
                    if c == '"' {
                        return Some(Ok(Token::STR(buf.into())));
                    } else {
                        buf.push(c);
                    }
                }

                return Some(Err(Error::Okay));
            },

            w if w.is_alphabetic() => {
                let mut word = String::new();
                word.push(w);
                while let Some(&w) = self.input.peek() {
                    if !in_ident(w) { break; }
                    word.push(w);
                    self.input.next();
                }

                match word.as_ref() {
                    "def" => Token::DEF,
                    "let" => Token::LET,
                    "return" => Token::RETURN,
                    _ => Token::ID(Ident::new(word).unwrap()),
                }
            },

            d if d.is_digit(10) => {
                let mut digits = String::new();
                digits.push(d);
                while let Some(&d) = self.input.peek() {
                    if !d.is_digit(10) { break; }
                    digits.push(d);
                    self.input.next();
                }

                Token::INT(digits.parse::<Int>().unwrap())
            },

            other => {
                return Some(Err(Error::UnimplementedToken { ch: other }));
            },
        }))
    }
}

fn in_ident(c: char) -> bool {
    c.is_alphabetic() || c.is_digit(10) || c == '_'
}

use std::fmt;

impl fmt::Display for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Token::DEF => write!(f, "def"),
            Token::LET => write!(f, "let"),
            Token::RETURN => write!(f, "return"),
            Token::EOL => write!(f, ";"),
            Token::COMMA => write!(f, ","),
            Token::COLON => write!(f, ":"),
            Token::EQUAL => write!(f, "="),
            Token::ADD => write!(f, "+"),
            Token::SUB => write!(f, "-"),
            Token::DIV => write!(f, "/"),
            Token::MUL => write!(f, "*"),
            Token::ID(ref id) => write!(f, "{}", id),
            Token::STR(ref s) => write!(f, "{:?}", s),
            Token::INT(i) => write!(f, "{}", i),
            Token::LPAR => write!(f, "("),
            Token::RPAR => write!(f, ")"),
            Token::LSQB => write!(f, "["),
            Token::RSQB => write!(f, "]"),
            Token::LCBR => write!(f, "{{"),
            Token::RCBR => write!(f, "}}"),
            Token::BEGIN => write!(f, "BEGIN"),
            Token::END => write!(f, "END"),
        }
    }
}

#[test]
fn syntax() {
    let src = "def foo(): return bar";
    let t = Tokenizer::new(src);
    let items = t.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(&items, &[
               Token::DEF,
               Token::ID(Ident::new("foo").unwrap()),
               Token::LPAR,
               Token::RPAR,
               Token::COLON,
               Token::RETURN,
               Token::ID(Ident::new("bar").unwrap()),
    ]);
}

#[test]
fn string() {
    let src = r#" "let's go" "#;
    let t = Tokenizer::new(src);
    t.collect::<Result<Vec<_>, _>>().unwrap();
}
