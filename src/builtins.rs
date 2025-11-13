use crate::vm::type_def::Arity;

// ========== 빌트인 함수 ID ==========
pub const BUILTIN_PRINT_ID: u8 = 0;
pub const BUILTIN_INPUT_ID: u8 = 1;
pub const BUILTIN_INT_ID: u8 = 2;
pub const BUILTIN_BOOL_ID: u8 = 3;
pub const BUILTIN_STR_ID: u8 = 4;
pub const BUILTIN_LEN_ID: u8 = 5;
pub const BUILTIN_RANGE_ID: u8 = 6;
pub const BUILTIN_FLOAT_ID: u8 = 7;
pub const BUILTIN_LIST_ID: u8 = 8;
pub const BUILTIN_DICT_ID: u8 = 9;
pub const BUILTIN_ASSERT_ID: u8 = 10;

// ========== 빌트인 타입 ID ==========
// 0-99는 builtin 타입, 100+는 사용자 정의 타입 (TYPE_USER_START는 type_def.rs에 정의)
pub const TYPE_INT: u16 = 0;
pub const TYPE_BOOL: u16 = 1;
pub const TYPE_STR: u16 = 2;
pub const TYPE_NONE: u16 = 3;
pub const TYPE_RANGE: u16 = 4;
pub const TYPE_LIST: u16 = 5;
pub const TYPE_DICT: u16 = 6;
pub const TYPE_FLOAT: u16 = 7;
pub const TYPE_FUNCTION: u16 = 8;

// ========== 빌트인 클래스 타입 ==========
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BuiltinClassType {
    /// `range(start, stop, step)` 타입
    Range,
    /// `[1, 2, 3]` 타입
    List,
    /// `{"a": 1}` 타입
    Dict,
    // 미래 확장:
    // Set,    // `{1, 2, 3}`
    // Tuple,  // `(1, 2, 3)`
}

impl BuiltinClassType {
    /// 타입 이름 반환
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::List => "list",
            Self::Dict => "dict",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub name: &'static str,
    pub arity: Arity,
    pub builtin_id: u8,
}

impl Builtin {
    pub fn check_arity(&self, got: usize) -> bool {
        self.arity.check(got)
    }
}

const PRINT: Builtin = Builtin {
    name: "print",
    arity: Arity::Variadic, // print() can take any number of arguments
    builtin_id: BUILTIN_PRINT_ID,
};

const INPUT: Builtin = Builtin {
    name: "input",
    arity: Arity::Range(0, 1), // input() or input(prompt)
    builtin_id: BUILTIN_INPUT_ID,
};

const INT: Builtin = Builtin {
    name: "int",
    arity: Arity::Exact(1), // int(x)
    builtin_id: BUILTIN_INT_ID,
};

const BOOL: Builtin = Builtin {
    name: "bool",
    arity: Arity::Exact(1), // bool(x)
    builtin_id: BUILTIN_BOOL_ID,
};

const STR: Builtin = Builtin {
    name: "str",
    arity: Arity::Exact(1), // str(x)
    builtin_id: BUILTIN_STR_ID,
};

const LEN: Builtin = Builtin {
    name: "len",
    arity: Arity::Exact(1), // len(x)
    builtin_id: BUILTIN_LEN_ID,
};

const RANGE: Builtin = Builtin {
    name: "range",
    arity: Arity::Range(1, 3), // range(stop) or range(start, stop) or range(start, stop, step)
    builtin_id: BUILTIN_RANGE_ID,
};

const FLOAT: Builtin = Builtin {
    name: "float",
    arity: Arity::Exact(1), // float(x)
    builtin_id: BUILTIN_FLOAT_ID,
};

const ASSERT: Builtin = Builtin {
    name: "assert",
    arity: Arity::Exact(1), // assert(condition)
    builtin_id: BUILTIN_ASSERT_ID,
};

// TODO: Uncomment when list() and dict() constructors are implemented
// const LIST: Builtin = Builtin {
//     name: "list",
//     arity: Arity::Range(0, 1), // list() or list(iterable)
//     builtin_id: BUILTIN_LIST_ID,
// };
//
// const DICT: Builtin = Builtin {
//     name: "dict",
//     arity: Arity::Exact(0), // dict() - no args initially
//     builtin_id: BUILTIN_DICT_ID,
// };

static REGISTRY: &[Builtin] = &[PRINT, INPUT, INT, BOOL, STR, LEN, RANGE, FLOAT, ASSERT];
// TODO: Add LIST and DICT to registry when implemented
// static REGISTRY: &[Builtin] = &[PRINT, INPUT, INT, BOOL, STR, LEN, RANGE, FLOAT, LIST, DICT, ASSERT];

pub fn all() -> &'static [Builtin] {
    REGISTRY
}

pub fn lookup(name: &str) -> Option<&'static Builtin> {
    REGISTRY.iter().find(|&b| b.name == name)
}

pub fn lookup_by_id(id: u8) -> Option<&'static Builtin> {
    REGISTRY.iter().find(|&b| b.builtin_id == id)
}
