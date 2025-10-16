#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [&'static str],
}

impl Builtin { pub const fn arity(&self) -> usize { self.params.len() } }

const PRINT: Builtin = Builtin { name: "print", params: &["value"] };
const INPUT: Builtin = Builtin { name: "input", params: &[] };
const INT: Builtin = Builtin { name: "int", params: &["x"] };
const BOOL: Builtin = Builtin { name: "bool", params: &["x"] };

static REGISTRY: &[Builtin] = &[PRINT, INPUT, INT, BOOL];

pub fn all() -> &'static [Builtin] { REGISTRY }

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    for b in REGISTRY { if b.name == name { return Some(b); } }
    None
}

// pub struct Builtin {
//     pub name: String,
//     pub params:
// }