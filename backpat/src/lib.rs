pub mod parse;
pub mod opcode;
pub mod compile;

use std::collections::BTreeMap;

pub type GroupNumber = u8;
pub type Captures = BTreeMap<GroupNumber, (usize, usize)>;

pub fn eq_ignore_case(lhs: char, rhs: char) -> bool {
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
mod scaffold {
    use std::iter::Peekable;
    use std::str::Chars;

    use parse::{TokenStream, Result};

    impl<'a> TokenStream<String> for Peekable<Chars<'a>> {
        fn getc(&mut self) -> Option<char> {
            self.next()
        }

        fn lookahead(&mut self) -> Option<char> {
            self.peek().cloned()
        }

        fn parse_payload(&mut self, _sigil: char) -> Result<String> {
            panic!("Variables not supported in test harness")
        }
    }
}

#[cfg(test)]
macro_rules! assert_match {
    ( $re:expr, $string:expr $(, $capture:expr )* ) => {{
        use parse::Ast;

        let re: &str = $re;
        let string: &str = $string;
        let expected: Vec<&str> = vec![ $( $capture ),* ];

        let pat = Ast::<String>::parse(&mut re.chars().peekable())
            .unwrap_or_else(|err| panic!("Parse failed: {}", err));

        let found = pat.translate().matches(string).unwrap_or_else(|| {
            panic!("Pattern {} does not match string {:?}", pat, string);
        });

        for (id, (left, right)) in found {
            if let Some(&expected) = expected.get(id as usize) {
                assert_eq!(expected, &string[left .. right]);
            }
        }
    }}
}

#[test]
fn backtracking() {
    assert_match!("/./", "the dot", "t");
    assert_match!("/\\w/", "word", "w");
    assert_match!("/\\w+/", "words", "words");
    assert_match!("/CASE/i", "case", "case");
    assert_match!("/.+b/", "aaabc", "aaab");
}
