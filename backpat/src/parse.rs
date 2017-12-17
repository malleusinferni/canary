use std::collections::HashSet;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ast<Payload> {
    pub root: Group<Payload>,
    pub ignore_case: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Group<Payload> {
    pub number: u8,
    pub branches: Vec<Branch<Payload>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Branch<Payload> {
    pub leaves: Vec<Leaf<Payload>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Leaf<Payload> {
    Group(Group<Payload>),
    Raw(String),
    Class(Class),
    AnchorStart,
    AnchorEnd,
    Repeat {
        prefix: Box<Leaf<Payload>>,
        times: Repeat,
        suffix: Branch<Payload>,
    },
    Payload(Payload),
}

pub trait TokenStream<Payload> {
    fn lookahead(&mut self) -> Option<char>;
    fn getc(&mut self) -> Option<char>;
    fn parse_payload(&mut self, char) -> Result<Payload>;
}

#[derive(Debug)]
pub enum Error {
    Bad,
}

pub type Result<T, E=Error> = ::std::result::Result<T, E>;

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

enum Item<Payload> {
    Leaf {
        leaf: Leaf<Payload>,
    },

    Repeat {
        prefix: Box<Leaf<Payload>>,
        times: Repeat,
    },
}

struct Tree<Payload> {
    items: Vec<Item<Payload>>,
}

impl<Payload> Ast<Payload> {
    pub fn parse<T: TokenStream<Payload>>(stream: &mut T) -> Result<Self> {
        let root = {
            let group_number = 0;

            let _marker = None;
            let mut parser = Parser { stream, group_number, _marker };

            let open = parser.consume()?;

            let close = match open {
                '(' => ')',
                '[' => ']',
                '{' => '}',
                '<' => '>',
                '/' => '/',
                '|' => '|',
                '"' => '"',
                _ => return Err(Error::Bad),
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
                _ => return Err(Error::Bad),
            }

            stream.getc();
        }

        Ok(Ast { root, ignore_case, })
    }
}

struct Parser<'a, P, T: 'a + TokenStream<P>> {
    stream: &'a mut T,
    group_number: u8,
    _marker: Option<Box<P>>,
}

impl<'a, P, T: TokenStream<P>> Parser<'a, P, T> {
    fn consume(&mut self) -> Result<char> {
        self.stream.getc().ok_or(Error::Bad)
    }

    fn lookahead(&mut self) -> Result<char> {
        self.stream.lookahead().ok_or(Error::Bad)
    }

    fn parse_group(&mut self, end: char) -> Result<Group<P>> {
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
                            return Err(Error::Bad);
                        }
                    }

                    let count = digits.parse::<usize>().map_err(|_| {
                        Error::Bad
                    })?;

                    tree.repeat(Repeat::Count(count))?;
                },

                '}' | ']' | ')' => {
                    // Unbalanced delimiters
                    return Err(Error::Bad);
                },

                '^' => {
                    tree.push(Leaf::AnchorStart);
                },

                '$' => {
                    let next = self.lookahead()?;

                    if next == end || next == ')' || next == '|' {
                        tree.push(Leaf::AnchorEnd);
                    } else if next.is_alphabetic() {
                        let payload = self.stream.parse_payload('$')?;
                        tree.push(Leaf::Payload(payload));
                    } else {
                        return Err(Error::Bad);
                    }
                },

                '%' => {
                    let payload = self.stream.parse_payload('%')?;
                    tree.push(Leaf::Payload(payload));
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
                            _ => return Err(Error::Bad),
                        }));
                    }
                },

                other if (other as u32) < 0x20 => {
                    return Err(Error::Bad);
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
                                return Err(Error::Bad)?;
                            }

                            for ch in prev .. next {
                                use std::char::from_u32;

                                let ch = from_u32(ch)
                                    .ok_or(Error::Bad)?;

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
}

impl<Payload> Tree<Payload> {
    fn push(&mut self, leaf: Leaf<Payload>) {
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
            Err(Error::Bad)
        }
    }

    fn take(&mut self) -> Result<Branch<Payload>> {
        use std::vec::Drain;

        fn get<P>(stream: &mut Drain<Item<P>>) -> Result<Branch<P>> {
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

    impl<Payload: Display> Display for Ast<Payload> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            let flags = if self.ignore_case { "i" } else { "" };
            write!(f, "re/{}/{}", &self.root, flags)
        }
    }

    impl<Payload: Display> Display for Group<Payload> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            let branches = self.branches.iter()
                .map(|branch| branch.to_string())
                .collect::<Vec<String>>();

            write!(f, "{}", branches.join("|"))
        }
    }

    impl<Payload: Display> Display for Branch<Payload> {
        fn fmt(&self, f: &mut Formatter) -> Result {
            for leaf in self.leaves.iter() {
                leaf.fmt(f)?;
            }

            Ok(())
        }
    }

    impl<Payload: Display> Display for Leaf<Payload> {
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

                Leaf::Payload(ref p) => {
                    write!(f, "{}", p)
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
