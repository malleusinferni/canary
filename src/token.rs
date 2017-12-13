use super::*;
use ident::*;
use value::*;
use pattern::*;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    NEARWORD(Ident),
    FARWORD(Ident),
    GLOBAL(Ident),
    VAR(Ident),
    SYM(Ident),
    INT(Int),
    STR(Str),
    PAT(Pattern),
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
    MATCH,
    DOT,
    NOT,
    EQ,
    NE,
    ADD,
    SUB,
    DIV,
    MUL,
    EOL,
    AND,
    OR,
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
    strings: Strings,
}

impl<'a> Tokenizer<'a> {
    pub fn new(src: &'a str) -> Self {
        Tokenizer {
            input: src.chars().peekable(),
            strings: Strings::new(),
        }
    }

    pub fn spanned(self) -> Spanned<Self> {
        Spanned { inner: self }
    }

    fn pattern(&mut self) -> Result<Pattern> {
        let err = || Error::InvalidRegex;

        let open = self.input.next().ok_or(err())?;

        let close = match open {
            '(' => ')',
            '[' => ']',
            '{' => '}',
            '<' => '>',
            '/' => '/',
            '|' => '|',
            '"' => '"',
            _ => return Err(err()),
        };

        let mut chars = String::new();

        while let Some(ch) = self.input.next() {
            if ch == close {
                let text = self.strings.intern(chars)?;
                return Ok(Pattern::Find(text));
            } else if ch == '\n' {
                return Err(err());
            } else {
                chars.push(ch);
            }
        }

        Err(err())
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

            ',' => Token::COMMA,
            ';' => Token::EOL,
            '.' => Token::DOT,

            '+' => Token::ADD,
            '-' => Token::SUB,
            '/' => Token::DIV,
            '*' => Token::MUL,

            '=' => if let Some(&'~') = self.input.peek() {
                self.input.next();
                Token::MATCH
            } else {
                Token::EQUAL
            },

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

            ':' => match self.input.peek().cloned() {
                Some(w) if w.is_alphabetic() => {
                    self.input.next();
                    let mut word = String::new();
                    word.push(w);

                    while let Some(&w) = self.input.peek() {
                        if !in_ident(w) { break; }
                        word.push(w);
                        self.input.next();
                    }

                    Token::SYM(self.strings.intern(word).unwrap())
                },

                _ => Token::COLON,
            },

            '$' => {
                let mut word = String::new();
                let w = self.input.next()?;

                if !w.is_alphabetic() {
                    unimplemented!("Special vars");
                }

                word.push(w);

                while let Some(&w) = self.input.peek() {
                    if !in_ident(w) { break; }
                    word.push(w);
                    self.input.next();
                }

                Token::VAR(self.strings.intern(word).unwrap())
            },

            '%' => {
                let mut word = String::new();
                let w = self.input.next()?;

                word.push(w);
                while let Some(&w) = self.input.peek() {
                    if !in_ident(w) { break; }
                    word.push(w);
                    self.input.next();
                }

                Token::GLOBAL(self.strings.intern(word).unwrap())
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
                    "not" => Token::NOT,
                    "eq" => Token::EQ,
                    "ne" => Token::NE,
                    "and" => Token::AND,
                    "or" => Token::OR,

                    "re" => return Some(self.pattern().map(|pat| {
                        Token::PAT(pat)
                    })),

                    _ => {
                        let ident = self.strings.intern(word).unwrap();
                        if self.input.peek() == Some(&'(') {
                            Token::NEARWORD(ident)
                        } else {
                            Token::FARWORD(ident)
                        }
                    },
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
            Token::NOT => write!(f, "not"),
            Token::EQ => write!(f, "eq"),
            Token::NE => write!(f, "ne"),
            Token::AND => write!(f, "and"),
            Token::OR => write!(f, "or"),
            Token::EOL => write!(f, ";"),
            Token::DOT => write!(f, "."),
            Token::COMMA => write!(f, ","),
            Token::COLON => write!(f, ":"),
            Token::EQUAL => write!(f, "="),
            Token::MATCH => write!(f, "=~"),
            Token::ADD => write!(f, "+"),
            Token::SUB => write!(f, "-"),
            Token::DIV => write!(f, "/"),
            Token::MUL => write!(f, "*"),
            Token::NEARWORD(ref id) => write!(f, "{}", id),
            Token::FARWORD(ref id) => write!(f, "{}", id),
            Token::GLOBAL(ref id) => write!(f, "%{}", id),
            Token::VAR(ref id) => write!(f, "${}", id),
            Token::SYM(ref id) => write!(f, ":{}", id),
            Token::STR(ref s) => write!(f, "{:?}", s),
            Token::INT(i) => write!(f, "{}", i),
            Token::PAT(ref p) => write!(f, "{}", p),
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
    let src = "sub foo() { return $bar; }";
    let mut t = Tokenizer::new(src);
    let foo = t.strings.intern("foo").unwrap();
    let bar = t.strings.intern("bar").unwrap();

    let items = t.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(&items, &[
               Token::DEF,
               Token::NEARWORD(foo),
               Token::LPAR,
               Token::RPAR,
               Token::LCBR,
               Token::RETURN,
               Token::VAR(bar),
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
