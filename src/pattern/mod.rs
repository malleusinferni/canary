pub mod parse;
pub mod compile;

use self::parse::*;

use super::*;
use value::Str;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Resolved(Ast<usize>),
}

pub type GroupNumber = u8;
pub type Captures = Vec<(GroupNumber, usize, usize)>;

impl Pattern {
    pub fn matches<E>(self, env: &mut E, haystack: &str) -> Option<Captures>
        where E: Env
    {
        let ast = match self { Pattern::Resolved(ast) => { ast }, };
        let Ast { ref root, ignore_case } = ast;

        let captures = vec![];

        let matcher = Matcher {
            env,
            haystack,
            captures,
            ignore_case,
            right: 0,
        };

        matcher.check_root(root)
    }
}

pub trait Env {
    fn read_local(&mut self, usize) -> Str;
    fn read_global(&mut self, &Ident) -> Option<Str>;
}

struct Matcher<'a, E: 'a + Env> {
    env: &'a mut E,
    haystack: &'a str,
    captures: Vec<(GroupNumber, usize, usize)>,
    ignore_case: bool,
    right: usize,
}

struct Checkpoint {
    right: usize,
    captures: usize,
}

impl<'a, E: Env> Matcher<'a, E> {
    fn check_root(mut self, root: &Group<usize>) -> Option<Captures> {
        let haystack = self.haystack;

        for (left, _) in haystack.char_indices() {
            self.haystack = &haystack[left ..];
            self.right = 0;

            if self.check_group(root) {
                return Some(self.captures);
            }
        }

        None
    }

    fn mark(&self) -> Checkpoint {
        let Matcher { right, .. } = *self;
        let captures = self.captures.len();
        Checkpoint { right, captures }
    }

    fn recall(&mut self, here: &Checkpoint) {
        let Checkpoint { right, captures } = *here;
        self.right = right;
        self.captures.drain(captures ..);
    }

    fn capture(&mut self, num: GroupNumber, here: Checkpoint) {
        if self.captures.iter().any(|&(n, _, _)| n == num) {
            return;
        }

        self.captures.push((num, here.right, self.right));
    }

    fn get_char(&mut self) -> Option<char> {
        self.haystack[self.right ..].chars().next().map(|ch| {
            self.right += ch.len_utf8();
            ch
        })
    }

    fn check_char(&mut self, needle: char) -> bool {
        self.get_char().map(|ch| {
            if self.ignore_case {
                eq_ignore_case(needle, ch)
            } else {
                needle == ch
            }
        }).unwrap_or(false)
    }

    fn check_str(&mut self, string: &str) -> bool {
        if self.ignore_case {
            string.chars().all(|ch| self.check_char(ch))
        } else if self.haystack[self.right ..].starts_with(string) {
            self.right += string.len();
            true
        } else {
            false
        }
    }

    fn check_group(&mut self, group: &Group<usize>) -> bool {
        let here = self.mark();

        for branch in group.branches.iter() {
            if self.check_branch(branch) {
                self.capture(group.number, here);
                return true;
            } else {
                self.recall(&here);
            }
        }

        false
    }

    fn check_branch(&mut self, branch: &Branch<usize>) -> bool {
        for leaf in branch.leaves.iter() {
            if !self.check_leaf(leaf) {
                return false;
            }
        }

        true
    }

    fn check_leaf(&mut self, leaf: &Leaf<usize>) -> bool {
        match *leaf {
            Leaf::AnchorStart => {
                self.right == 0
            },

            Leaf::AnchorEnd => {
                self.right == self.haystack.len()
            },

            Leaf::Group(ref group) => {
                self.check_group(group)
            },

            Leaf::Raw(ref s) => {
                self.check_str(s)
            },

            Leaf::Class(ref class) => {
                self.check_class(class)
            },

            Leaf::Repeat { ref prefix, times, ref suffix } => {
                self.repeat(prefix, times, suffix)
            },

            Leaf::Local { name } => {
                let string = self.env.read_local(name);
                self.check_str(&string)
            },

            Leaf::Global { ref name } => {
                if let Some(string) = self.env.read_global(name) {
                    self.check_str(&string)
                } else {
                    false
                }
            },
        }
    }

    fn repeat(&mut self, prefix: &Leaf<usize>, times: Repeat, suffix: &Branch<usize>) -> bool {
        let (min, max) = match times {
            Repeat::OneOrZero => (0, Some(1)),
            Repeat::ZeroOrMore => (0, None),
            Repeat::OneOrMore => (1, None),
            Repeat::Count(n) => (n, Some(n)),
        };

        let max = max.unwrap_or(self.haystack[self.right ..].len());

        for _ in 0 .. min {
            if !self.check_leaf(prefix) {
                return false;
            }
        }

        let mut stack = vec![];

        for _ in min .. max {
            stack.push(self.mark());

            if !(self.check_leaf(prefix) && self.check_branch(suffix)) {
                break;
            }
        }

        while let Some(here) = stack.pop() {
            self.recall(&here);

            if !self.check_leaf(prefix) {
                self.recall(&here);
                continue;
            }

            if self.check_branch(suffix) {
                return true;
            }
        }

        self.check_branch(suffix)
    }

    fn check_class(&mut self, class: &Class) -> bool {
        let ch = match self.get_char() {
            Some(cr) => cr,
            None => return false,
        };

        match *class {
            Class::Dot => {
                true
            },

            Class::Digit => {
                ch.is_digit(10)
            },

            Class::Word => {
                ch.is_alphabetic()
            },

            Class::Space => {
                ch.is_whitespace()
            },

            Class::Custom { ref members, invert } => if invert {
                members.iter().all(|&m| ch != m)
            } else {
                members.iter().any(|&m| ch == m)
            },
        }
    }
}

fn eq_ignore_case(lhs: char, rhs: char) -> bool {
    if lhs == rhs {
        return true;
    }

    let mut lhs = lhs.to_lowercase();
    let mut rhs = rhs.to_lowercase();

    loop {
        match (lhs.next(), rhs.next()) {
            (None, None) => return true,
            (lhs, rhs) => if lhs != rhs { return false },
        }
    }
}

#[test]
fn check_ignore_case() {
    let pairs = &[
        ('a', true, 'a'),
        ('A', true, 'A'),
        ('a', true, 'A'),
        ('a', false, 'b'),
    ];

    for &(lhs, equal, rhs) in pairs {
        if equal {
            assert!(eq_ignore_case(lhs, rhs), "{} == {}", lhs, rhs);
        } else {
            assert!(!eq_ignore_case(lhs, rhs), "{} != {}", lhs, rhs);
        }
    }
}

mod display {
    use std::fmt::{Display, Formatter, Result};

    use super::*;

    impl Display for Pattern {
        fn fmt(&self, f: &mut Formatter) -> Result {
            match *self {
                Pattern::Resolved(ref ast) => ast.fmt(f),
            }
        }
    }
}
