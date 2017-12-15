use std::collections::HashSet;

use {Result, Error};

use super::*;

use ident::*;

use token::Tokenizer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ast<Local> {
    pub root: Group<Local>,
    pub ignore_case: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group<Local> {
    pub number: u8,
    pub branches: Vec<Branch<Local>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch<Local> {
    pub leaves: Vec<Leaf<Local>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Leaf<Local> {
    Group(Group<Local>),
    Raw(String),
    Class(Class),
    AnchorStart,
    AnchorEnd,
    Repeat {
        prefix: Box<Leaf<Local>>,
        times: Repeat,
        suffix: Branch<Local>,
    },
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

enum Item {
    Leaf {
        leaf: Leaf<Ident>,
    },

    Repeat {
        prefix: Box<Leaf<Ident>>,
        times: Repeat,
    },
}

struct Tree {
    items: Vec<Item>,
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

    fn parse_group(&mut self, end: char) -> Result<Group<Ident>> {
        let number = self.group_number;
        self.group_number += 1;

        let mut branches = vec![];
        let mut tree = Tree { items: vec![] };

        loop {
            let ch = self.consume()?;

            if ch == end {
                branches.push(tree.take()?);
                return Ok(Group { number, branches });
            }

            match ch {
                '|' => {
                    branches.push(tree.take()?);
                },

                '(' => {
                    tree.push(Leaf::Group(self.parse_group(')')?));
                },

                '[' => {
                    tree.push(self.parse_class().map(Leaf::Class)?);
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

                    tree.repeat(Repeat::Count(count))?;
                },

                '}' | ']' | ')' => {
                    // Unbalanced delimiters
                    return Err(Error::InvalidRegex);
                },

                '^' => {
                    tree.push(Leaf::AnchorStart);
                },

                '$' => {
                    let next = self.lookahead()?;

                    if next == end || next == ')' || next == '|' {
                        tree.push(Leaf::AnchorEnd);
                    } else if next.is_alphabetic() {
                        let name = self.parse_ident()?;
                        tree.push(Leaf::Local { name });
                    } else {
                        return Err(Error::InvalidRegex);
                    }
                },

                '%' => {
                    let name = self.parse_ident()?;
                    tree.push(Leaf::Global { name });
                },

                '.' => {
                    tree.push(Leaf::Class(Class::Dot));
                },

                '+' => {
                    tree.repeat(Repeat::OneOrMore)?;
                },

                '*' => {
                    tree.repeat(Repeat::ZeroOrMore)?;
                },

                '?' => {
                    tree.repeat(Repeat::OneOrZero)?;
                },

                '\\' => {
                    let c = self.consume()?;

                    if c == end || "|()[]{}.^$?*+\\".contains(c) {
                        tree.putchar(c);
                    } else {
                        tree.push(Leaf::Class(match c {
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
                    tree.putchar(other);
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

impl Tree {
    fn push(&mut self, leaf: Leaf<Ident>) {
        self.items.push(Item::Leaf { leaf });
    }

    fn putchar(&mut self, ch: char) {
        if let Some(string) = self.last_mut() {
            string.push(ch);

            // Early return instead of "else" so the borrow checker
            // will understand what we're doing here
            return;
        }

        let mut string = String::new();
        string.push(ch);
        self.push(Leaf::Raw(string));
    }

    fn last_mut(&mut self) -> Option<&mut String> {
        self.items.last_mut().and_then(|item| match *item {
            Item::Leaf { ref mut leaf } => Some(leaf),
            _ => None,
        }).and_then(|leaf| match *leaf {
            Leaf::Raw(ref mut string) => Some(string),
            _ => None,
        })
    }

    fn repeat(&mut self, times: Repeat) -> Result<()> {
        if let Some(Item::Leaf { leaf }) = self.items.pop() {
            self.items.push(Item::Repeat {
                times,
                prefix: Box::new(leaf),
            });
            Ok(())
        } else {
            Err(Error::InvalidRegex)
        }
    }

    fn take(&mut self) -> Result<Branch<Ident>> {
        use std::vec::Drain;

        fn get(stream: &mut Drain<Item>) -> Result<Branch<Ident>> {
            let mut leaves = vec![];

            while let Some(item) = stream.next() {
                match item {
                    Item::Leaf { leaf } => {
                        leaves.push(leaf);
                    },

                    Item::Repeat { prefix, times } => {
                        let suffix = get(stream)?;

                        leaves.push(Leaf::Repeat {
                            prefix,
                            times,
                            suffix,
                        });
                    },
                }
            }

            Ok(Branch { leaves })
        }

        get(&mut self.items.drain(..))
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
            let branches = self.branches.iter()
                .map(|branch| branch.to_string())
                .collect::<Vec<String>>();

            write!(f, "{}", branches.join("|"))
        }
    }

    impl<Local: Display> Display for Branch<Local> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            for leaf in self.leaves.iter() {
                leaf.fmt(f)?;
            }

            Ok(())
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

                Leaf::Repeat { ref prefix, times, ref suffix } => {
                    write!(f, "{}{}", prefix, match times {
                        Repeat::OneOrZero => "?",
                        Repeat::OneOrMore => "+",
                        Repeat::ZeroOrMore => "*",
                        Repeat::Count(_) => "{...}",
                    })?;

                    suffix.fmt(f)
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
