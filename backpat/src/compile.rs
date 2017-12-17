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

            Leaf::Repeat { ref prefix, times, ref suffix } => {
                let prefix = Box::new(prefix.map(f)?);

                let suffix = {
                    let leaves = suffix.leaves.iter().map(|leaf| {
                        leaf.map(f)
                    }).collect::<Result<Vec<_>, E>>()?;

                    Branch { leaves }
                };

                Leaf::Repeat { prefix, times, suffix }
            },

            Leaf::Raw(ref string) => Leaf::Raw(string.clone()),

            Leaf::Class(ref class) => Leaf::Class(class.clone()),

            Leaf::AnchorStart => Leaf::AnchorStart,
            Leaf::AnchorEnd => Leaf::AnchorEnd,
        })
    }
}
