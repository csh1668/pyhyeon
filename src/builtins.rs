#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub params: &'static [&'static str],
    /// Minimum number of required arguments (for optional parameters)
    pub min_arity: usize,
}

impl Builtin {
    pub const fn arity(&self) -> usize {
        self.params.len()
    }

    pub const fn min_arity(&self) -> usize {
        self.min_arity
    }

    pub const fn max_arity(&self) -> usize {
        self.params.len()
    }
}

const PRINT: Builtin = Builtin {
    name: "print",
    params: &["value"],
    min_arity: 0, // Allow print() with no arguments
};
const INPUT: Builtin = Builtin {
    name: "input",
    params: &["prompt"],
    min_arity: 0, // prompt is optional
};
const INT: Builtin = Builtin {
    name: "int",
    params: &["x"],
    min_arity: 1,
};
const BOOL: Builtin = Builtin {
    name: "bool",
    params: &["x"],
    min_arity: 1,
};

const STR: Builtin = Builtin {
    name: "str",
    params: &["x"],
    min_arity: 1,
};

const LEN: Builtin = Builtin {
    name: "len",
    params: &["x"],
    min_arity: 1,
};

const RANGE: Builtin = Builtin {
    name: "range",
    params: &["start", "stop", "step"],
    min_arity: 1, // range(stop) or range(start, stop) or range(start, stop, step)
};

static REGISTRY: &[Builtin] = &[PRINT, INPUT, INT, BOOL, STR, LEN, RANGE];

pub fn all() -> &'static [Builtin] {
    REGISTRY
}

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    REGISTRY.iter().find(|&b| b.name == name).map(|v| v as _)
}
