pub mod parse;
pub mod compile;

use self::parse::*;

pub type GroupNumber = u8;
pub type Captures = Vec<(GroupNumber, usize, usize)>;

impl<P: AsRef<str>> Ast<P> {
    pub fn matches(&self, haystack: &str) -> Option<Captures> {
        let Ast { ref root, ignore_case } = *self;

        let captures = vec![];

        let matcher = Matcher {
            haystack,
            captures,
            ignore_case,
            right: 0,
        };

        matcher.check_root(root)
    }
}

struct Matcher<'a> {
    haystack: &'a str,
    captures: Vec<(GroupNumber, usize, usize)>,
    ignore_case: bool,
    right: usize,
}

struct Checkpoint {
    right: usize,
    captures: usize,
}

impl<'a> Matcher<'a> {
    fn check_root<P: AsRef<str>>(mut self, root: &Group<P>) -> Option<Captures> {
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

    fn check_group<P: AsRef<str>>(&mut self, group: &Group<P>) -> bool {
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

    fn check_branch<P: AsRef<str>>(&mut self, branch: &Branch<P>) -> bool {
        for leaf in branch.leaves.iter() {
            if !self.check_leaf(leaf) {
                return false;
            }
        }

        true
    }

    fn check_leaf<P: AsRef<str>>(&mut self, leaf: &Leaf<P>) -> bool {
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

            Leaf::Payload(ref string) => {
                self.check_str(string.as_ref())
            },
        }
    }

    fn repeat<P: AsRef<str>>(&mut self, prefix: &Leaf<P>, times: Repeat, suffix: &Branch<P>) -> bool {
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

            if !self.check_leaf(prefix) {
                break;
            }
        }

        while let Some(here) = stack.pop() {
            if self.check_branch(suffix) {
                return true;
            }

            self.recall(&here);
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

#[cfg(test)]
macro_rules! assert_match {
    ( $re:expr, $string:expr $(, $capture:expr )* ) => {{
        use token::{Token, Tokenizer};

        let re: &str = $re;
        let string: &str = $string;
        let expected: Vec<&str> = vec![ $( $capture ),* ];

        let ast = match Tokenizer::new(re).next() {
            Some(Ok(Token::PAT(ast))) => ast,
            _ => panic!("Failed to parse pattern: {}", re),
        };

        let pat: Pattern = Arc::new(ast.map(|_| {
            panic!("Variables not supported")
        }).unwrap());

        let found = pat.matches(string).unwrap_or_else(|| {
            panic!("Pattern {} does not match string {:?}", ast, string);
        });

        for (id, left, right) in found {
            if let Some(&expected) = expected.get(id as usize) {
                assert_eq!(expected, &string[left .. right]);
            }
        }
    }}
}

#[test]
fn backtracking() {
    assert_match!("re/./", "the dot", "t");
    assert_match!("re/\\w/", "word", "w");
    assert_match!("re/\\w+/", "words", "words");
    assert_match!("re/CASE/i", "case", "case");
    assert_match!("re/.+b/", "aaabc", "aaab");
}
