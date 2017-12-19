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
