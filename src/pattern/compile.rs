use std::collections::HashSet;

use Result;

use super::parse::*;

use ident::*;

pub struct NamesUsed {
    pub locals: HashSet<Ident>,
    pub globals: HashSet<Ident>,
}

impl<In> Ast<In> {
    pub fn map_locals<Out, F>(&self, f: F) -> Result<Ast<Out>>
        where F: Fn(&In) -> Result<Out>
    {
        let Ast { ref root, ignore_case } = *self;
        let root = root.map(&f)?;
        Ok(Ast { root, ignore_case })
    }
}

impl<In> Group<In> {
    fn map<Out, F>(&self, f: &F) -> Result<Group<Out>>
        where F: Fn(&In) -> Result<Out>
    {
        let branches = self.branches.iter().map(|branch| {
            let leaves = branch.leaves.iter().map(|leaf| {
                leaf.map(f)
            }).collect::<Result<Vec<_>>>()?;
            Ok(Branch { leaves })
        }).collect::<Result<Vec<_>>>()?;
        Ok(Group { branches })
    }
}

impl<In> Leaf<In> {
    fn map<Out, F>(&self, f: &F) -> Result<Leaf<Out>>
        where F: Fn(&In) -> Result<Out>
    {
        Ok(match *self {
            Leaf::Local { ref name } => Leaf::Local {
                name: f(name)?,
            },

            Leaf::Global { ref name } => Leaf::Global {
                name: name.clone(),
            },

            Leaf::Group(ref group) => Leaf::Group(group.map(f)?),

            Leaf::Repeat(ref leaf, repeat) => {
                let leaf = Box::new(leaf.map(f)?);
                Leaf::Repeat(leaf, repeat)
            },

            Leaf::Raw(ref string) => Leaf::Raw(string.clone()),

            Leaf::Class(ref class) => Leaf::Class(class.clone()),

            Leaf::AnchorStart => Leaf::AnchorStart,
            Leaf::AnchorEnd => Leaf::AnchorEnd,
        })
    }
}
