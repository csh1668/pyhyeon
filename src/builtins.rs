#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [&'static str],
}

impl Builtin {
    pub const fn arity(&self) -> usize {
        self.params.len()
    }
}

const PRINT: Builtin = Builtin {
    name: "print",
    params: &["value"],
};
const INPUT: Builtin = Builtin {
    name: "input",
    params: &[],
};
const INT: Builtin = Builtin {
    name: "int",
    params: &["x"],
};
const BOOL: Builtin = Builtin {
    name: "bool",
    params: &["x"],
};

const STR: Builtin = Builtin {
    name: "str",
    params: &["x"],
};

const LEN: Builtin = Builtin {
    name: "len",
    params: &["x"],
};

static REGISTRY: &[Builtin] = &[PRINT, INPUT, INT, BOOL, STR, LEN];

pub fn all() -> &'static [Builtin] {
    REGISTRY
}

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    REGISTRY.iter().find(|&b| b.name == name).map(|v| v as _)
}

// pub struct Builtin {
//     pub name: String,
//     pub params:
// }
