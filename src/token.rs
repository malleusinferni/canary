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
    IF,
    ELSE,
    WHILE,
    COLON,
    RETURN,
    EQUAL,
    COMMA,
    ADD,
    SUB,
    DIV,
    MUL,
    EOL,
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
}

impl<'a> Tokenizer<'a> {
    pub fn new(src: &'a str) -> Self {
        Tokenizer {
            input: src.chars().peekable(),
        }
    }

    pub fn spanned(self) -> Spanned<Self> {
        Spanned { inner: self }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(&s) = self.input.peek() {
            if s == '#' {
                while let Some(n) = self.input.next() {
                    if n == '\n' {
                        return Some(Ok(Token::EOL));
                    }
                }
            } else if s.is_whitespace() {
                let _ = self.input.next();
                continue;
            } else {
                break;
            }
        }

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

                return Some(Err(Error::MalformedString));
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
                    "sub" => Token::DEF,
                    "my" => Token::LET,
                    "if" => Token::IF,
                    "else" => Token::ELSE,
                    "while" => Token::WHILE,
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
            Token::DEF => write!(f, "sub"),
            Token::LET => write!(f, "my"),
            Token::IF => write!(f, "if"),
            Token::ELSE => write!(f, "else"),
            Token::WHILE => write!(f, "while"),
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
        }
    }
}

#[test]
fn syntax() {
    let src = "sub foo() { return bar; }";
    let t = Tokenizer::new(src);
    let items = t.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(&items, &[
               Token::DEF,
               Token::ID(Ident::new("foo").unwrap()),
               Token::LPAR,
               Token::RPAR,
               Token::LCBR,
               Token::RETURN,
               Token::ID(Ident::new("bar").unwrap()),
               Token::EOL,
               Token::RCBR,
    ]);
}

#[test]
fn string() {
    let src = r#" "let's go" "#;
    let t = Tokenizer::new(src);
    t.collect::<Result<Vec<_>, _>>().unwrap();
}
