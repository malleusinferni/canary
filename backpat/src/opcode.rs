use super::{GroupNumber, Captures, eq_ignore_case};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Op<Label> {
    MOV { ix: usize },
    POINT { sp: usize, },
    MARK { label: Label },
    LOOP { label: Label },
    JUMP { label: Label },
    JNZ { label: Label },
    FAIL,
    LEFT { group: GroupNumber },
    RIGHT,
    BEGIN,
    END,
    DOT,
    WORD,
    DIGIT,
    SPACE,
    STR { index: usize },
    ANY { index: usize },
    NONE { index: usize },
}

pub struct Eval<'a> {
    code: &'a ::compile::Compiled,
    haystack: &'a str,
    captures: Vec<(Delim, usize)>,
    indices: Vec<usize>,
    marks: Vec<Checkpoint>,
    right: usize,
    pc: usize,
    sp: usize,
    ch: char,
    ic: bool,
}

struct Checkpoint {
    pc: usize,
    ch: char,
    sp: usize,
    ix: usize,
    right: usize,
    captures_len: usize,
}

enum Delim {
    Left(GroupNumber),
    Right,
}

impl<'a> Eval<'a> {
    pub fn new(code: &'a ::compile::Compiled, haystack: &'a str) -> Self {
        let indices = vec![0; code.loop_count()];

        Eval {
            code,
            indices,
            haystack,
            marks: vec![],
            captures: vec![],
            right: 0,
            pc: 0,
            sp: 0,
            ch: '\0',
            ic: code.ignore_case,
        }
    }

    pub fn eval(mut self) -> Option<Captures> {
        let haystack = self.haystack;

        for (left, _) in haystack.char_indices() {
            self.marks.clear();
            self.captures.clear();
            self.pc = 0;
            self.sp = 0;
            self.right = 0;
            self.haystack = &haystack[left ..];

            if self.eval_once() {
                let mut captures = Captures::new();
                let mut stack = vec![];

                for (delim, index) in self.captures.drain(..) {
                    match delim {
                        Delim::Left(group) => stack.push((group, index)),

                        Delim::Right => {
                            let (group, left) = stack.pop().unwrap();
                            let span = (left, index);

                            if captures.contains_key(&group) {
                                continue;
                            }

                            captures.insert(group, span);
                        },
                    }
                }

                return Some(captures);
            }
        }

        None
    }

    fn eval_once(&mut self) -> bool {
        while let Some(op) = self.code.fetch(self.pc) {
            self.pc += 1;

            if self.step(op) {
                continue;
            }

            if let Some(cp) = self.marks.pop() {
                let Checkpoint {
                    pc,
                    ch,
                    sp,
                    ix,
                    right,
                    captures_len,
                } = cp;

                self.pc = pc;
                self.ch = ch;
                self.sp = sp;
                self.right = right;
                self.indices[sp] = ix;
                self.captures.drain(captures_len ..);
            } else {
                return false;
            }
        }

        true
    }

    fn mark(&mut self, pc: usize) {
        let Eval { ch, sp, right, .. } = *self;
        let ix = self.indices[sp];
        let captures_len = self.captures.len();

        self.marks.push(Checkpoint {
            pc, ch, sp, ix, right, captures_len
        });
    }

    fn bump(&mut self) -> bool {
        self.haystack[self.right ..].chars().next().map(|ch| {
            self.right += ch.len_utf8();
            self.ch = ch;
        }).is_some()
    }

    fn check_char(&mut self, ch: char) -> bool {
        if self.ic {
            eq_ignore_case(ch, self.ch)
        } else {
            ch == self.ch
        }
    }

    fn step(&mut self, op: Op<usize>) -> bool {
        match op {
            Op::MARK { label } => {
                self.mark(label);

                true
            },

            Op::POINT { sp } => {
                self.sp = sp;

                true
            },

            Op::MOV { ix } => {
                self.indices[self.sp] = ix;

                true
            },

            Op::LOOP { label } => {
                let ix = self.indices[self.sp].saturating_sub(1);
                self.indices[self.sp] = ix;

                if ix > 0 {
                    self.pc = label;
                }

                true
            },

            Op::FAIL => {
                false
            },

            Op::LEFT { group } => {
                let delim = Delim::Left(group);
                self.captures.push((delim, self.right));

                true
            },

            Op::RIGHT => {
                let delim = Delim::Right;
                self.captures.push((delim, self.right));

                true
            },

            Op::JUMP { label } => {
                self.pc = label;

                true
            },

            Op::JNZ { label } => {
                if self.indices[self.sp] != 0 {
                    self.pc = label;
                }

                true
            },

            Op::BEGIN => {
                self.right == 0
            },

            Op::END => {
                self.right == self.haystack.len()
            },

            Op::DOT => {
                self.bump()
            },

            Op::WORD => {
                self.bump() && self.ch.is_alphabetic()
            },

            Op::DIGIT => {
                self.bump() && self.ch.is_digit(10)
            },

            Op::SPACE => {
                self.bump() && self.ch.is_whitespace()
            },

            Op::STR { index } => {
                self.code.string(index).chars().all(|ch| {
                    self.bump() && self.check_char(ch)
                })
            },

            Op::ANY { index } => {
                self.bump() && self.code.string(index).chars().any(|ch| {
                    self.check_char(ch)
                })
            },

            Op::NONE { index } => {
                self.bump() && !self.code.string(index).chars().any(|ch| {
                    self.check_char(ch)
                })
            },
        }
    }
}
