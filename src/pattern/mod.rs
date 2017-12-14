pub mod parse;
pub mod compile;

use self::parse::*;

use super::*;
use value::Str;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Resolved(Ast<usize>),
}

impl Pattern {
    pub fn matches<E: Env>(self, env: &mut E, haystack: &str) -> bool {
        let ast = match self { Pattern::Resolved(ast) => { ast }, };
        let Ast { ref root, ignore_case } = ast;

        let mut matcher = Matcher {
            env,
            haystack,
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
    ignore_case: bool,
    right: usize,
}

impl<'a, E: Env> Matcher<'a, E> {
    fn check_root(&mut self, root: &Group<usize>) -> bool {
        let haystack = self.haystack;

        for (left, _) in haystack.char_indices() {
            self.haystack = &haystack[left ..];
            self.right = 0;

            println!("Checking {:?}...", self.haystack);

            if self.check_group(root) {
                return true;
            }
        }

        false
    }

    fn get_char(&mut self) -> Option<char> {
        self.haystack.chars().next().map(|ch| {
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
        if self.haystack[self.right ..].starts_with(string) {
            self.right += string.len();
            true
        } else {
            false
        }
    }

    fn check_group(&mut self, group: &Group<usize>) -> bool {
        let Matcher { right, .. } = *self;

        for branch in group.branches.iter() {
            if self.check_branch(branch) {
                return true;
            } else {
                // Backtrack
                self.right = right;
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

            Leaf::Repeat(ref leaf, count) => {
                self.repeat_leaf(leaf, count)
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

    fn repeat_leaf(&mut self, leaf: &Leaf<usize>, count: Repeat) -> bool {
        let (min, max) = match count {
            Repeat::OneOrZero => (0, Some(1)),
            Repeat::ZeroOrMore => (0, None),
            Repeat::OneOrMore => (1, None),
            Repeat::Count(n) => (n, Some(n)),
        };

        let mut good = false;

        let limit = self.haystack.len().saturating_sub(self.right);

        for i in 0 .. limit {
            if let Some(max) = max {
                if i >= max {
                    break;
                }
            }

            if i >= min {
                good = true;
            }

            if !self.check_leaf(leaf) {
                return false;
            }
        }

        good
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
        ('a', 'a'),
        ('A', 'A'),
    ];

    for &(lhs, rhs) in pairs {
        assert!(eq_ignore_case(lhs, rhs), "{} != {}", lhs, rhs);
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
