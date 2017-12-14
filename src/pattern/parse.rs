use std::collections::HashSet;

use {Result, Error};

use super::*;

use ident::*;

use token::Tokenizer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ast {
    pub root: Group,
    pub ignore_case: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group {
    pub branches: Vec<Branch>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch {
    pub leaves: Vec<Leaf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Leaf {
    Group(Group),
    Raw(String),
    AnchorStart,
    AnchorEnd,
    ClassDot,
    ClassDigit,
    ClassWord,
    ClassCustom {
        invert: bool,
        members: HashSet<char>,
    },
    Local {
        name: Ident,
    },
    Global {
        name: Ident,
    },
}

impl Pattern {
    pub fn parse(stream: &mut Tokenizer) -> Result<Self> {
        let root ={
            let mut parser = Parser { stream };

            let open = parser.consume()?;

            let close = match open {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                '<' => '>',
                '/' => '/',
                '|' => '|',
                '"' => '"',
                _ => return Err(Error::InvalidRegex),
            };

            parser.parse_group(close)?
        };

        let mut ignore_case = false;

        while let Some(c) = stream.lookahead() {
            if !c.is_alphabetic() {
                break;
            }

            match c {
                'i' => ignore_case = true,
                _ => return Err(Error::InvalidRegex),
            }

            stream.getc();
        }

        Ok(Pattern::Deferred(Ast {
            root,
            ignore_case,
        }))
    }
}

struct Parser<'a, 'b : 'a> {
    stream: &'a mut Tokenizer<'b>,
}

impl<'a, 'b : 'a> Parser<'a, 'b> {
    fn consume(&mut self) -> Result<char> {
        self.stream.getc().ok_or(Error::InvalidRegex)
    }

    fn lookahead(&mut self) -> Result<char> {
        self.stream.lookahead().ok_or(Error::InvalidRegex)
    }

    fn parse_group(&mut self, end: char) -> Result<Group> {
        let mut branches = vec![];
        let mut leaves = vec![];

        fn putc(leaves: &mut Vec<Leaf>, ch: char) {
            if let Some(&mut Leaf::Raw(ref mut string)) = leaves.last_mut() {
                string.push(ch);

                // Early return instead of "else" so the borrow checker
                // will understand what we're doing here
                return;
            }

            let mut string = String::new();
            string.push(ch);
            leaves.push(Leaf::Raw(string));
        }

        loop {
            let ch = self.consume()?;

            if ch == end {
                branches.push(Branch { leaves });
                return Ok(Group { branches });
            }

            match ch {
                '|' => {
                    let leaves = leaves.drain(..).collect();
                    branches.push(Branch { leaves });
                },

                '(' => {
                    leaves.push(Leaf::Group(self.parse_group(')')?));
                },

                '[' => {
                    leaves.push(self.parse_class()?);
                },

                ']' | ')' => {
                    // Unbalanced delimiters
                    return Err(Error::InvalidRegex);
                },

                '$' => {
                    let next = self.lookahead()?;

                    if next == end || next == ')' || next == '|' {
                        leaves.push(Leaf::AnchorEnd);
                    } else if next.is_alphabetic() {
                        let name = self.parse_ident()?;
                        leaves.push(Leaf::Local { name });
                    } else {
                        return Err(Error::InvalidRegex);
                    }
                },

                '.' => {
                    leaves.push(Leaf::ClassDot);
                },

                '\\' => {
                    let c = self.consume()?;

                    if c == end || "(|).$".contains(c) {
                        putc(&mut leaves, c);
                    } else if c == 'd' {
                        leaves.push(Leaf::ClassDigit);
                    } else {
                        return Err(Error::InvalidRegex);
                    }
                },

                other if (other as u32) < 0x20 => {
                    return Err(Error::InvalidRegex);
                },

                other => {
                    putc(&mut leaves, other);
                },
            }
        }
    }

    fn parse_class(&mut self) -> Result<Leaf> {
        let mut prev = None;
        let mut invert = false;
        let mut members = HashSet::new();

        loop {
            let ch = self.consume()?;

            match ch {
                ']' => break,

                '^' if prev.is_none() => {
                    invert = true;
                    continue;
                },

                '-' => {
                    let next = self.lookahead()?;

                    if next != ']' {
                        if let Some(prev) = prev.take() {
                            self.consume()?;

                            let prev = prev as u32;
                            let next = next as u32;

                            for ch in prev .. next {
                                use std::char::from_u32;

                                let ch = from_u32(ch)
                                    .ok_or(Error::InvalidRegex)?;

                                members.insert(ch);
                            }

                            continue;
                        }
                    }

                    members.insert('-');
                },

                _ => {
                    members.insert(ch);
                },
            }

            prev = Some(ch);
        }

        Ok(Leaf::ClassCustom { invert, members })
    }

    fn parse_ident(&mut self) -> Result<Ident> {
        self.stream.word().unwrap_or(Err(Error::InvalidRegex))
    }
}
