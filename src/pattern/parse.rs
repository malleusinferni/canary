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
    Repeat(Box<Leaf>, Repeat),
    Local {
        name: Ident,
    },
    Global {
        name: Ident,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Repeat {
    OneOrZero,
    ZeroOrMore,
    OneOrMore,
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
        let mut branch = Branch { leaves: vec![] };

        loop {
            let ch = self.consume()?;

            if ch == end {
                branches.push(branch);
                return Ok(Group { branches });
            }

            match ch {
                '|' => {
                    branches.push(branch.take());
                },

                '(' => {
                    branch.push(Leaf::Group(self.parse_group(')')?));
                },

                '[' => {
                    branch.push(self.parse_class()?);
                },

                ']' | ')' => {
                    // Unbalanced delimiters
                    return Err(Error::InvalidRegex);
                },

                '$' => {
                    let next = self.lookahead()?;

                    if next == end || next == ')' || next == '|' {
                        branch.push(Leaf::AnchorEnd);
                    } else if next.is_alphabetic() {
                        let name = self.parse_ident()?;
                        branch.push(Leaf::Local { name });
                    } else {
                        return Err(Error::InvalidRegex);
                    }
                },

                '.' => {
                    branch.push(Leaf::ClassDot);
                },

                '+' => {
                    branch.repeat(Repeat::OneOrMore)?;
                },

                '*' => {
                    branch.repeat(Repeat::ZeroOrMore)?;
                },

                '?' => {
                    branch.repeat(Repeat::OneOrZero)?;
                },

                '\\' => {
                    let c = self.consume()?;

                    if c == end || "(|).$".contains(c) {
                        branch.putchar(c);
                    } else if c == 'd' {
                        branch.push(Leaf::ClassDigit);
                    } else {
                        return Err(Error::InvalidRegex);
                    }
                },

                other if (other as u32) < 0x20 => {
                    return Err(Error::InvalidRegex);
                },

                other => {
                    branch.putchar(other);
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

impl Branch {
    fn push(&mut self, leaf: Leaf) {
        self.leaves.push(leaf);
    }

    fn putchar(&mut self, ch: char) {
        if let Some(&mut Leaf::Raw(ref mut string)) = self.leaves.last_mut() {
            string.push(ch);

            // Early return instead of "else" so the borrow checker
            // will understand what we're doing here
            return;
        }

        let mut string = String::new();
        string.push(ch);
        self.push(Leaf::Raw(string));
    }

    fn repeat(&mut self, kind: Repeat) -> Result<()> {
        let last = self.leaves.pop().ok_or(Error::InvalidRegex)?;
        self.push(Leaf::Repeat(last.into(), kind));
        Ok(())
    }

    fn take(&mut self) -> Self {
        let leaves = self.leaves.drain(..).collect();
        Branch { leaves }
    }
}
