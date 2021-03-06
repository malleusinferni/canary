use super::*;
use ident::*;
use value::*;

use pattern;

use backpat::GroupNumber;

#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    NEARWORD(Ident),
    FARWORD(Ident),
    GLOBAL(Ident),
    GROUP(u8),
    VAR(Ident),
    SYM(Ident),
    INT(Int),
    STR(Vec<Interp>),
    PAT(pattern::Ast),
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
    ASSERT,
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

#[derive(Clone, Debug, PartialEq)]
pub enum Interp {
    S(Str),
    V(Ident),
    G(Ident),
    C(GroupNumber),
}

pub struct Spanned<'a> {
    inner: Tokenizer<'a>,
}

impl<'a> Iterator for Spanned<'a> {
    type Item = Result<(usize, Token, usize)>;

    fn next(&mut self) -> Option<Self::Item> {
        let left = self.inner.right;

        self.inner.next().map(|result| match result {
            Ok(t) => {
                Ok((left, t, self.inner.right))
            },

            Err(mut err) => {
                if let Some((line, col)) = self.inner.line_and_col() {
                    err = Error::WithPosition {
                        cause: err.into(),
                        line: line,
                        column: col,
                    };
                }

                Err(err)
            },
        })
    }
}

pub struct Tokenizer<'a> {
    src: &'a str,
    input: Peekable<Chars<'a>>,
    strings: Strings,
    left: usize,
    right: usize,
}

impl<'a> Tokenizer<'a> {
    pub fn new(src: &'a str) -> Self {
        Tokenizer {
            src,
            input: src.chars().peekable(),
            strings: Strings::new(),
            left: 0,
            right: 0,
        }
    }

    pub fn spanned(self) -> Spanned<'a> {
        Spanned { inner: self }
    }

    pub fn line_and_col(&self) -> Option<(usize, usize)> {
        if self.left >= self.src.len() {
            return None;
        }

        let mut line = 0;
        let mut col = 0;

        for (i, c) in self.src.char_indices() {
            if i > self.left {
                break;
            }

            if c == '\n' {
                line += 1;
                col = 0;
            } else {
                col += 1;
            }
        }

        Some((line, col))
    }

    pub fn lookahead(&mut self) -> Option<char> {
        self.input.peek().cloned()
    }

    pub fn getc(&mut self) -> Option<char> {
        let next = self.input.next();

        if let Some(c) = next {
            self.left = self.right;
            self.right = self.left + c.len_utf8();
        }

        next
    }

    pub fn word(&mut self) -> Option<Result<Ident>> {
        self.getc().map(|c| {
            self.endword(c)
        })
    }

    fn endword(&mut self, start: char) -> Result<Ident> {
        let mut word = String::new();

        word.push(start);

        while let Some(c) = self.lookahead() {
            if in_ident(c) {
                word.push(c);
                self.getc();
            } else {
                break;
            }
        }

        self.strings.intern(word)
    }

    fn interp(&mut self) -> Result<Token> {
        let err = || Error::MalformedString;

        let mut items = Vec::new();

        while let Some(ch) = self.getc() {
            match ch {
                '"' => return Ok(Token::STR(items)),

                '$' => {
                    let c = self.lookahead().ok_or(err())?;

                    if c.is_digit(10) {
                        let mut digits = String::new();
                        while let Some(c) = self.lookahead() {
                            if c.is_digit(10) {
                                digits.push(c);
                                self.getc();
                            } else {
                                break;
                            }
                        }

                        let num = digits.parse::<GroupNumber>()
                            .map_err(|_| err())?;

                        items.push(Interp::C(num));
                    } else {
                        let word = self.word().unwrap_or(Err(err()))?;
                        items.push(Interp::V(word));
                    }
                },

                '%' => {
                    let word = self.word().unwrap_or(Err(err()))?;
                    items.push(Interp::G(word));
                },

                other => {
                    let mut s = String::new();
                    s.push(other);

                    while let Some(c) = self.lookahead() {
                        if "$%\"".contains(c) { break; }

                        self.getc();

                        if c == '\\' {
                            s.push(self.unescape()?);
                        } else {
                            s.push(c);
                        }
                    }

                    items.push(Interp::S(self.strings.intern(s)?));
                },
            }
        }

        Err(err())
    }

    fn unescape(&mut self) -> Result<char> {
        Ok(match self.getc().ok_or(Error::MalformedString)? {
            '$' => '$',
            '%' => '%',
            '"' => '"',
            'n' => '\n',
            'r' => '\r',
            't' => '\t',
            '\\' => '\\',
            _ => return Err(Error::InvalidEscape),
        })
    }

    fn token(&mut self, first: char) -> Result<Token> {
        Ok(match first {
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

            '=' => if let Some('~') = self.lookahead() {
                self.getc();
                Token::MATCH
            } else {
                Token::EQUAL
            },

            '"' => self.interp()?,

            ':' => match self.lookahead() {
                Some(w) if w.is_alphabetic() => {
                    Token::SYM(self.word().ok_or(Error::Eof)??)
                },

                _ => Token::COLON,
            },

            '$' => {
                let w = self.getc().ok_or(Error::Eof)?;
                let mut word = String::new();
                word.push(w);

                if w.is_digit(10) {
                    Token::GROUP(word.parse::<u8>().unwrap())
                } else if w.is_alphabetic() {
                    Token::VAR(self.endword(w)?)
                } else {
                    return Err(Error::UnimplementedToken { ch: w });
                }
            },

            '%' => {
                Token::GLOBAL(self.word().ok_or(Error::Eof)??)
            },

            w if w.is_alphabetic() => {
                let word = self.endword(w)?;

                match word.as_ref() {
                    "sub" => Token::DEF,
                    "my" => Token::LET,
                    "if" => Token::IF,
                    "else" => Token::ELSE,
                    "while" => Token::WHILE,
                    "return" => Token::RETURN,
                    "assert" => Token::ASSERT,
                    "not" => Token::NOT,
                    "eq" => Token::EQ,
                    "ne" => Token::NE,
                    "and" => Token::AND,
                    "or" => Token::OR,

                    "re" => return pattern::Ast::parse(self).map(|pat| {
                        Token::PAT(pat)
                    }).map_err(|_| Error::InvalidRegex),

                    _ => {
                        if self.lookahead() == Some('(') {
                            Token::NEARWORD(word)
                        } else {
                            Token::FARWORD(word)
                        }
                    },
                }
            },

            d if d.is_digit(10) => {
                let mut digits = String::new();
                digits.push(d);
                while let Some(d) = self.lookahead() {
                    if !d.is_digit(10) { break; }
                    digits.push(d);
                    self.getc();
                }

                Token::INT(digits.parse::<Int>().unwrap())
            },

            other => {
                return Err(Error::UnimplementedToken { ch: other });
            },
        })
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(s) = self.lookahead() {
            if s == '#' {
                while self.getc()? != '\n' {
                    continue;
                }
            } else if s.is_whitespace() {
                self.getc();
                continue;
            } else {
                break;
            }
        }

        self.getc().map(|first| self.token(first))
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
            Token::ASSERT => write!(f, "assert"),
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
            Token::GROUP(num) => write!(f, "${}", num),
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
fn various() {
    let sources = &[
        "$foo $1 $9 $0 $ok_what_now $UPPERCASE",
        "1 23 456 7890",
        " spaces # Comment at end\n",
    ];

    for src in sources {
        Tokenizer::new(src).collect::<Result<Vec<_>>>().unwrap_or_else(|err| {
            println!("Failed to tokenize: {}", src);
            println!("Error: {}", err);
            panic!("Test failed");
        });
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
    let strings = &[
        r#" "let's go" "#,
        r#" "okay $friend" "#,
        r#" "hello\nworld" "#,
    ];

    for string in strings {
        let t = Tokenizer::new(string);
        t.collect::<Result<Vec<_>, _>>().unwrap();
    }
}

#[test]
fn pattern() {
    let patterns = &[
        "re/(hello|world)/",
        "re/(hello|world)/i",
        "re/^hello[ ]world$/",
    ];

    for src in patterns {
        let tokens = Tokenizer::new(src)
            .collect::<Result<Vec<_>>>()
            .unwrap();

        assert_eq!(tokens.len(), 1);
    }
}
