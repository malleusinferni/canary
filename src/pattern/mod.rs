pub mod parse;
pub mod compile;

use self::parse::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Resolved(Ast<usize>),
}

impl Pattern {
    pub fn matches(self, haystack: &str) -> bool {
        let ast = match self { Pattern::Resolved(ast) => { ast }, };
        let Ast { ref root, ignore_case } = ast;

        let mut matcher = Matcher {
            haystack,
            ignore_case,
            left: 0,
            right: 0,
        };

        matcher.check_group(root)
    }
}

struct Matcher<'a> {
    haystack: &'a str,
    ignore_case: bool,
    left: usize,
    right: usize,
}

impl<'a> Matcher<'a> {
    fn check_group(&mut self, group: &Group<usize>) -> bool {
        let Matcher { left, right, .. } = *self;

        group.branches.iter().any(|branch| {
            self.left = left;
            self.right = right;
            self.check_branch(branch)
        })
    }

    fn check_branch(&mut self, branch: &Branch<usize>) -> bool {
        branch.leaves.iter().all(|leaf| {
            self.check_leaf(leaf)
        })
    }

    fn check_leaf(&mut self, leaf: &Leaf<usize>) -> bool {
        match *leaf {
            Leaf::AnchorStart => {
                self.left == 0
            },

            Leaf::AnchorEnd => {
                self.right == self.haystack.len()
            },

            Leaf::Raw(ref s) => {
                let needle = s.chars();
                let haystack = self.haystack[self.right ..].chars();

                let yes = needle.zip(haystack).all(|(n, h)| {
                    if self.ignore_case {
                        n.to_lowercase().count() == h.to_lowercase().count()
                            && n.to_lowercase().zip(h.to_lowercase())
                            .all(|(n, h)| n == h)
                    } else {
                        n == h
                    }
                });

                if yes { self.right += s.len() }

                yes
            },

            _ => unimplemented!(),
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
