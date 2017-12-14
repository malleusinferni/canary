use std::collections::HashSet;

use {Result, Error};

use super::*;

use ident::*;

use token::Tokenizer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ast<Local=Ident> {
    pub root: Group<Local>,
    pub ignore_case: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group<Local=Ident> {
    pub number: u8,
    pub branches: Vec<Branch<Local>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch<Local=Ident> {
    pub leaves: Vec<Leaf<Local>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Leaf<Local=Ident> {
    Group(Group<Local>),
    Raw(String),
    Class(Class),
    AnchorStart,
    AnchorEnd,
    Repeat(Box<Leaf<Local>>, Repeat),
    Local {
        name: Local,
    },
    Global {
        name: Ident,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Class {
    Dot,
    Digit,
    Word,
    Space,
    Custom {
        invert: bool,
        members: HashSet<char>,
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Repeat {
    OneOrZero,
    ZeroOrMore,
    OneOrMore,
    Count(usize),
}

impl Pattern {
    pub fn parse(stream: &mut Tokenizer) -> Result<Ast<Ident>> {
        let root = {
            let group_number = 0;
            let mut parser = Parser { stream, group_number };

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

        Ok(Ast { root, ignore_case, })
    }
}

struct Parser<'a, 'b : 'a> {
    stream: &'a mut Tokenizer<'b>,
    group_number: u8,
}

impl<'a, 'b : 'a> Parser<'a, 'b> {
    fn consume(&mut self) -> Result<char> {
        self.stream.getc().ok_or(Error::InvalidRegex)
    }

    fn lookahead(&mut self) -> Result<char> {
        self.stream.lookahead().ok_or(Error::InvalidRegex)
    }

    fn parse_group(&mut self, end: char) -> Result<Group> {
        let number = self.group_number;
        self.group_number += 1;

        let mut branches = vec![];
        let mut branch = Branch { leaves: vec![] };

        loop {
            let ch = self.consume()?;

            if ch == end {
                branches.push(branch);
                return Ok(Group { number, branches });
            }

            match ch {
                '|' => {
                    branches.push(branch.take());
                },

                '(' => {
                    branch.push(Leaf::Group(self.parse_group(')')?));
                },

                '[' => {
                    branch.push(self.parse_class().map(Leaf::Class)?);
                },

                '{' => {
                    let mut digits = String::new();

                    loop {
                        let d = self.consume()?;

                        if d.is_digit(10) {
                            digits.push(d);
                        } else if d == '}' {
                            break;
                        } else {
                            return Err(Error::InvalidRegex);
                        }
                    }

                    let count = digits.parse::<usize>().map_err(|_| {
                        Error::InvalidRegex
                    })?;

                    branch.repeat(Repeat::Count(count))?;
                },

                '}' | ']' | ')' => {
                    // Unbalanced delimiters
                    return Err(Error::InvalidRegex);
                },

                '^' => {
                    branch.push(Leaf::AnchorStart);
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

                '%' => {
                    let name = self.parse_ident()?;
                    branch.push(Leaf::Global { name });
                },

                '.' => {
                    branch.push(Leaf::Class(Class::Dot));
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

                    if c == end || "|()[]{}.^$?*+\\".contains(c) {
                        branch.putchar(c);
                    } else {
                        branch.push(Leaf::Class(match c {
                            'd' => Class::Digit,
                            'w' => Class::Word,
                            's' => Class::Space,
                            _ => return Err(Error::InvalidRegex),
                        }));
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

    fn parse_class(&mut self) -> Result<Class> {
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

                            if prev >= next {
                                return Err(Error::InvalidRegex)?;
                            }

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

        Ok(Class::Custom { invert, members })
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

fn magic(ch: char) -> bool {
    "()[]{}|.?+*/^$\\".contains(ch)
}

mod display {
    use super::*;
    use std::fmt::{Display, Formatter, Result};

    impl<Local: Display> Display for Ast<Local> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            let flags = if self.ignore_case { "i" } else { "" };
            write!(f, "re/{}/{}", &self.root, flags)
        }
    }

    impl<Local: Display> Display for Group<Local> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            let branches = self.branches.iter().map(|branch| {
                let mut buf = String::new();

                for leaf in &branch.leaves {
                    buf += &leaf.to_string();
                }

                buf
            }).collect::<Vec<String>>();

            write!(f, "{}", branches.join("|"))
        }
    }

    impl<Local: Display> Display for Leaf<Local> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Leaf::Group(ref group) => write!(f, "({})", group),

                Leaf::Raw(ref string) => {
                    for ch in string.chars() {
                        if magic(ch) { write!(f, "\\")?; }
                        write!(f, "{}", ch)?;
                    }

                    Ok(())
                },

                Leaf::AnchorStart => write!(f, "^"),
                Leaf::AnchorEnd => write!(f, "$"),

                Leaf::Class(ref class) => class.fmt(f),

                Leaf::Repeat(ref leaf, kind) => {
                    write!(f, "{}{}", leaf, match kind {
                        Repeat::OneOrZero => "?",
                        Repeat::OneOrMore => "+",
                        Repeat::ZeroOrMore => "*",
                        Repeat::Count(_) => "{...}",
                    })
                },

                Leaf::Local { ref name } => {
                    write!(f, "${}", name)
                },

                Leaf::Global { ref name } => {
                    write!(f, "%{}", name)
                },
            }
        }
    }

    impl Display for Class {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Class::Dot => write!(f, "."),
                Class::Digit => write!(f, "\\d"),
                Class::Space => write!(f, "\\s"),
                Class::Word => write!(f, "\\w"),
                Class::Custom { ref members, invert } => {
                    let members = members.iter().collect::<String>();
                    if invert {
                        write!(f, "[^{}]", members)
                    } else {
                        write!(f, "[{}]", members)
                    }
                },
            }
        }
    }
}
