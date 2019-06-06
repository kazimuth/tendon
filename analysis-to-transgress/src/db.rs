use rls_data::{Analysis, Def, Id};
use std::collections::HashMap;

static NO_KIDS: &'static [Id] = &[];

pub struct Children(HashMap<Id, Vec<Id>>);
impl Children {
    pub fn new(analysis: &Analysis) -> Children {
        let mut result = Children(HashMap::new());
        for def in &analysis.defs {
            def.parent.map(|parent| result.add(parent, def.id));
        }
        result
    }

    pub fn children(&self, id: Id) -> &[Id] {
        self.0.get(&id).map(|v| &v[..]).unwrap_or(NO_KIDS)
    }

    fn add(&mut self, parent: Id, child: Id) {
        self.0.entry(parent).or_insert_with(|| vec![]).push(child);
    }
}

pub struct Defs<'a>(pub HashMap<Id, &'a Def>);
impl Defs<'_> {
    pub fn new(analysis: &Analysis) -> Defs {
        let mut result = Defs(HashMap::new());

        for def in &analysis.defs {
            result.0.insert(def.id, &def);
        }

        result
    }
}
