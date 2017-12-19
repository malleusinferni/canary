use super::parse::*;

impl<In> Ast<In> {
    pub fn map<Out, E, F>(&self, mut f: F) -> Result<Ast<Out>, E>
        where F: FnMut(&In) -> Result<Out, E>
    {
        let Ast { ref root, ignore_case } = *self;
        let root = root.map(&mut f)?;
        Ok(Ast { root, ignore_case })
    }
}

impl<In> Group<In> {
    fn map<Out, E, F>(&self, f: &mut F) -> Result<Group<Out>, E>
        where F: FnMut(&In) -> Result<Out, E>
    {
        let number = self.number;
        let branches = self.branches.iter().map(|branch| {
            let leaves = branch.leaves.iter().map(|leaf| {
                leaf.map(f)
            }).collect::<Result<Vec<_>, E>>()?;
            Ok(Branch { leaves })
        }).collect::<Result<Vec<_>, E>>()?;
        Ok(Group { branches, number })
    }
}

impl<In> Leaf<In> {
    fn map<Out, E, F>(&self, f: &mut F) -> Result<Leaf<Out>, E>
        where F: FnMut(&In) -> Result<Out, E>
    {
        Ok(match *self {
            Leaf::Payload(ref var) => Leaf::Payload(f(var)?),

            Leaf::Group(ref group) => Leaf::Group(group.map(f)?),

            Leaf::Repeat { ref prefix, times } => {
                let prefix = Box::new(prefix.map(f)?);
                Leaf::Repeat { prefix, times }
            },

            Leaf::Raw(ref string) => Leaf::Raw(string.clone()),

            Leaf::Class(ref class) => Leaf::Class(class.clone()),

            Leaf::AnchorStart => Leaf::AnchorStart,
            Leaf::AnchorEnd => Leaf::AnchorEnd,
        })
    }
}

use std::collections::BTreeMap;

use opcode::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Compiled {
    code: Vec<Op<usize>>,
    index_space: usize,
    pub ignore_case: bool,
}

impl Compiled {
    pub fn matches(&self, haystack: &str) -> Option<super::Captures> {
        Eval::new(self, haystack).eval()
    }

    pub fn fetch(&self, pc: usize) -> Option<Op<usize>> {
        self.code.get(pc).cloned()
    }

    pub fn loop_count(&self) -> usize {
        self.index_space
    }
}

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
struct Sym(usize);

struct Compiler {
    code: Vec<Op<Sym>>,
    next_sym: usize,
    next_sp: usize,
    labels: BTreeMap<Sym, usize>,
}

impl Ast<String> {
    pub fn translate(&self) -> Compiled {
        let mut compiler = Compiler {
            code: vec![],
            next_sym: 0,
            next_sp: 0,
            labels: BTreeMap::new(),
        };

        let Ast { ignore_case, ref root } = *self;

        compiler.tr_group(root);

        let index_space = compiler.next_sp;
        let Compiler { code, labels, .. } = compiler;

        let code = code.into_iter().map(|op| match op {
            Op::MARK { label } => Op::MARK { label: labels[&label] },
            Op::LOOP { label } => Op::LOOP { label: labels[&label] },
            Op::JUMP { label } => Op::JUMP { label: labels[&label] },
            Op::JNZ { label } => Op::JNZ { label: labels[&label] },

            Op::POINT { sp } => Op::POINT { sp },
            Op::MOV { ix } => Op::MOV { ix },
            Op::CHAR { ch } => Op::CHAR { ch },
            Op::LEFT { group } => Op::LEFT { group },
            Op::RIGHT => Op::RIGHT,
            Op::BEGIN => Op::BEGIN,
            Op::END => Op::END,
            Op::DOT => Op::DOT,
            Op::DIGIT => Op::DIGIT,
            Op::WORD => Op::WORD,
            Op::SPACE => Op::SPACE,
            Op::FAIL => Op::FAIL,
        }).collect::<Vec<Op<usize>>>();

        Compiled { code, index_space, ignore_case }
    }
}

impl Compiler {
    fn emit(&mut self, op: Op<Sym>) {
        self.code.push(op);
    }

    fn label(&mut self, label: Sym) {
        let pc = self.code.len();
        self.labels.insert(label, pc);
    }

    fn gensym(&mut self) -> Sym {
        let sym = Sym(self.next_sym);
        self.next_sym += 1;
        sym
    }

    fn tr_group(&mut self, group: &Group<String>) {
        let Group { number, ref branches } = *group;

        self.emit(Op::LEFT { group: number });

        let succeed = self.gensym();

        let mut labels = vec![];
        for _ in 0 .. branches.len() {
            labels.push(self.gensym());
        }

        for (i, branch) in branches.iter().enumerate() {
            self.label(labels[i]);

            if let Some(&next) = labels.get(i + 1) {
                self.emit(Op::MARK { label: next });
            }

            let Branch { ref leaves } = *branch;

            for leaf in leaves.iter() {
                self.tr_leaf(leaf);
            }

            self.emit(Op::JUMP { label: succeed });
        }

        self.label(succeed);
        self.emit(Op::RIGHT);
    }

    fn tr_leaf(&mut self, leaf: &Leaf<String>) {
        match *leaf {
            Leaf::AnchorStart => self.emit(Op::BEGIN),

            Leaf::AnchorEnd => self.emit(Op::END),

            Leaf::Group(ref group) => self.tr_group(group),

            Leaf::Raw(ref string) => self.tr_string(string),

            Leaf::Payload(ref string) => self.tr_string(string),

            Leaf::Class(Class::Dot) => self.emit(Op::DOT),

            Leaf::Class(Class::Digit) => self.emit(Op::DIGIT),

            Leaf::Class(Class::Word) => self.emit(Op::WORD),

            Leaf::Class(Class::Space) => self.emit(Op::SPACE),

            Leaf::Class(Class::Custom { .. }) => {
                unimplemented!()
            },

            Leaf::Repeat { ref prefix, times, .. } => {
                self.tr_repeat(prefix, times)
            },
        }
    }

    fn tr_repeat(&mut self, prefix: &Leaf<String>, times: Repeat) {
        let (min, max) = match times {
            Repeat::OneOrZero => (0, Some(1)),
            Repeat::ZeroOrMore => (0, None),
            Repeat::OneOrMore => (1, None),
            Repeat::Count(u) => (u, Some(u)),
        };

        let max = max.unwrap_or(usize::max_value());

        let sp = self.next_sp;
        self.next_sp += 1;

        let loop1 = self.gensym();
        self.emit(Op::POINT { sp });
        self.emit(Op::MOV { ix: min });
        self.label(loop1);
        self.tr_leaf(prefix);
        self.emit(Op::POINT { sp });
        self.emit(Op::LOOP { label: loop1 });

        let loop2 = self.gensym();
        let exit = self.gensym();
        self.emit(Op::MOV { ix: max });
        self.label(loop2);
        self.emit(Op::MARK { label: exit });
        self.tr_leaf(prefix);
        self.emit(Op::POINT { sp });
        self.emit(Op::LOOP { label: loop2 });

        self.label(exit);
    }

    fn tr_string(&mut self, string: &str) {
        for ch in string.chars() {
            self.emit(Op::CHAR { ch });
        }
    }
}
