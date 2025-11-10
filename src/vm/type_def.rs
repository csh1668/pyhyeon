//! - **TypeDef**: 각 타입의 메타데이터 (이름, 메서드, 플래그)
//! - **MethodImpl**: 메서드 구현 방식 (Native vs UserDefined)
//! - **NativeMethod**: Rust로 구현된 메서드 ID
//! - **Arity**: 메서드 인자 개수 검증

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub methods: HashMap<String, MethodImpl>,

    /// 부모 타입 (상속 지원, 미래 기능)
    pub base_type: Option<u16>,

    pub flags: TypeFlags,
}

impl TypeDef {
    pub fn new(name: impl Into<String>, flags: TypeFlags) -> Self {
        Self {
            name: name.into(),
            methods: HashMap::new(),
            base_type: None,
            flags,
        }
    }

    pub fn add_method(&mut self, name: impl Into<String>, method: MethodImpl) {
        self.methods.insert(name.into(), method);
    }

    pub fn with_methods(mut self, methods: Vec<(&str, MethodImpl)>) -> Self {
        for (name, method) in methods {
            self.add_method(name, method);
        }
        self
    }
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct TypeFlags: u32 {
        /// 생성 후 수정할 수 없는 타입
        const IMMUTABLE = 1 << 0;

        /// `obj(args...)` 형태로 호출할 수 있는 타입
        const CALLABLE  = 1 << 1;

        /// `for x in obj:` 형태로 반복할 수 있는 타입
        const ITERABLE  = 1 << 2;
    }
}

#[derive(Debug, Clone)]
pub enum MethodImpl {
    Native { func: NativeMethod, arity: Arity },

    UserDefined { func_id: u16 },
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NativeMethod {
    // ========== Int 매직 메서드들 ==========
    IntAdd,
    IntSub,
    IntMul,
    IntFloorDiv,
    IntTrueDiv,
    IntMod,
    IntNeg,
    IntPos,
    IntLt,
    IntLe,
    IntGt,
    IntGe,
    IntEq,
    IntNe,

    // ========== Float 매직 메서드들 ==========
    FloatAdd,
    FloatSub,
    FloatMul,
    FloatTrueDiv,
    FloatFloorDiv,
    FloatMod,
    FloatNeg,
    FloatPos,
    FloatLt,
    FloatLe,
    FloatGt,
    FloatGe,
    FloatEq,
    FloatNe,

    // ========== String 매직 메서드들 ==========
    StrAdd, // concatenation
    StrMul, // repetition
    StrLt,
    StrLe,
    StrGt,
    StrGe,
    StrEq,
    StrNe,

    // ========== String 일반 메서드들 ==========
    StrUpper,
    StrLower,
    StrStrip,
    StrSplit,
    StrJoin,
    StrReplace,
    StrStartsWith,
    StrEndsWith,
    StrFind,
    StrCount,

    // ========== Range 메서드들 ==========
    RangeIter,
    RangeHasNext,
    RangeNext,

    // ========== List 메서드들 ==========
    ListAppend,
    ListPop,
    ListExtend,
    ListInsert,
    ListRemove,
    ListReverse,
    ListSort,
    ListClear,
    ListIndex,
    ListCount,
    ListIter,
    ListHasNext,
    ListNext,

    // ========== Dict 메서드들 ==========
    DictGet,
    DictKeys,
    DictValues,
    DictItems,
    DictPop,
    DictUpdate,
    DictClear,
    DictIter,
    DictHasNext,
    DictNext,
}

impl NativeMethod {
    /// 메서드 이름 가져오기
    pub fn name(&self) -> &'static str {
        match self {
            // Int 매직 메서드
            Self::IntAdd => "__add__",
            Self::IntSub => "__sub__",
            Self::IntMul => "__mul__",
            Self::IntFloorDiv => "__floordiv__",
            Self::IntTrueDiv => "__truediv__",
            Self::IntMod => "__mod__",
            Self::IntNeg => "__neg__",
            Self::IntPos => "__pos__",
            Self::IntLt => "__lt__",
            Self::IntLe => "__le__",
            Self::IntGt => "__gt__",
            Self::IntGe => "__ge__",
            Self::IntEq => "__eq__",
            Self::IntNe => "__ne__",

            // Float 매직 메서드
            Self::FloatAdd => "__add__",
            Self::FloatSub => "__sub__",
            Self::FloatMul => "__mul__",
            Self::FloatTrueDiv => "__truediv__",
            Self::FloatFloorDiv => "__floordiv__",
            Self::FloatMod => "__mod__",
            Self::FloatNeg => "__neg__",
            Self::FloatPos => "__pos__",
            Self::FloatLt => "__lt__",
            Self::FloatLe => "__le__",
            Self::FloatGt => "__gt__",
            Self::FloatGe => "__ge__",
            Self::FloatEq => "__eq__",
            Self::FloatNe => "__ne__",

            // String 매직 메서드
            Self::StrAdd => "__add__",
            Self::StrMul => "__mul__",
            Self::StrLt => "__lt__",
            Self::StrLe => "__le__",
            Self::StrGt => "__gt__",
            Self::StrGe => "__ge__",
            Self::StrEq => "__eq__",
            Self::StrNe => "__ne__",

            // String 일반 메서드
            Self::StrUpper => "upper",
            Self::StrLower => "lower",
            Self::StrStrip => "strip",
            Self::StrSplit => "split",
            Self::StrJoin => "join",
            Self::StrReplace => "replace",
            Self::StrStartsWith => "startswith",
            Self::StrEndsWith => "endswith",
            Self::StrFind => "find",
            Self::StrCount => "count",

            // Range 메서드
            Self::RangeIter => "__iter__",
            Self::RangeHasNext => "__has_next__",
            Self::RangeNext => "__next__",

            // List 메서드
            Self::ListAppend => "append",
            Self::ListPop => "pop",
            Self::ListExtend => "extend",
            Self::ListInsert => "insert",
            Self::ListRemove => "remove",
            Self::ListReverse => "reverse",
            Self::ListSort => "sort",
            Self::ListClear => "clear",
            Self::ListIndex => "index",
            Self::ListCount => "count",
            Self::ListIter => "__iter__",
            Self::ListHasNext => "__has_next__",
            Self::ListNext => "__next__",

            // Dict 메서드
            Self::DictGet => "get",
            Self::DictKeys => "keys",
            Self::DictValues => "values",
            Self::DictItems => "items",
            Self::DictPop => "pop",
            Self::DictUpdate => "update",
            Self::DictClear => "clear",
            Self::DictIter => "__iter__",
            Self::DictHasNext => "__has_next__",
            Self::DictNext => "__next__",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arity {
    /// 정확히 N개의 인자만 허용
    Exact(usize),

    /// min ~ max 개의 인자 허용
    Range(usize, usize),

    /// 가변 인자 (임의 개수)
    Variadic,
}

impl Arity {
    pub fn check(&self, got: usize) -> bool {
        match self {
            Arity::Exact(n) => got == *n,
            Arity::Range(min, max) => got >= *min && got <= *max,
            Arity::Variadic => true,
        }
    }

    pub fn description(&self) -> String {
        match self {
            Arity::Exact(n) => format!("{}", n),
            Arity::Range(min, max) if min == max => format!("{}", min),
            Arity::Range(min, max) => format!("{}-{}", min, max),
            Arity::Variadic => "any".to_string(),
        }
    }
}

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
    pub fn name(&self) -> &'static str {
        match self {
            Self::Range => "range",
            Self::List => "list",
            Self::Dict => "dict",
        }
    }
}

// ========== 타입 ID 상수 ==========
// 0-99는 builtin 타입, 100+는 사용자 정의 타입
pub const TYPE_INT: u16 = 0;
pub const TYPE_BOOL: u16 = 1;
pub const TYPE_STR: u16 = 2;
pub const TYPE_NONE: u16 = 3;
pub const TYPE_RANGE: u16 = 4;
pub const TYPE_LIST: u16 = 5;
pub const TYPE_DICT: u16 = 6;
pub const TYPE_FLOAT: u16 = 7;
pub const TYPE_USER_START: u16 = 100;

// ========== 유틸리티 함수 Re-exports ==========
// 실제 구현은 vm::utils 모듈에 있음
pub use super::utils::{make_string, make_list};

/// Built-in 타입들 초기화
///
/// 모든 built-in 타입(Int, Bool, String, None, Range, List, Dict)의
/// TypeDef를 생성하고 매직 메서드를 등록합니다.
pub fn init_builtin_types() -> Vec<TypeDef> {
    vec![
        // TYPE_INT (0)
        TypeDef::new("int", TypeFlags::IMMUTABLE).with_methods(vec![
            (
                "__add__",
                MethodImpl::Native {
                    func: NativeMethod::IntAdd,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__sub__",
                MethodImpl::Native {
                    func: NativeMethod::IntSub,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__mul__",
                MethodImpl::Native {
                    func: NativeMethod::IntMul,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__floordiv__",
                MethodImpl::Native {
                    func: NativeMethod::IntFloorDiv,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__truediv__",
                MethodImpl::Native {
                    func: NativeMethod::IntTrueDiv,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__mod__",
                MethodImpl::Native {
                    func: NativeMethod::IntMod,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__neg__",
                MethodImpl::Native {
                    func: NativeMethod::IntNeg,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__pos__",
                MethodImpl::Native {
                    func: NativeMethod::IntPos,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__lt__",
                MethodImpl::Native {
                    func: NativeMethod::IntLt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__le__",
                MethodImpl::Native {
                    func: NativeMethod::IntLe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__gt__",
                MethodImpl::Native {
                    func: NativeMethod::IntGt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ge__",
                MethodImpl::Native {
                    func: NativeMethod::IntGe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__eq__",
                MethodImpl::Native {
                    func: NativeMethod::IntEq,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ne__",
                MethodImpl::Native {
                    func: NativeMethod::IntNe,
                    arity: Arity::Exact(1),
                },
            ),
        ]),
        // TYPE_BOOL (1)
        TypeDef::new("bool", TypeFlags::IMMUTABLE),
        // TYPE_STR (2)
        TypeDef::new("str", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE).with_methods(vec![
            // 매직 메서드
            (
                "__add__",
                MethodImpl::Native {
                    func: NativeMethod::StrAdd,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__mul__",
                MethodImpl::Native {
                    func: NativeMethod::StrMul,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__lt__",
                MethodImpl::Native {
                    func: NativeMethod::StrLt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__le__",
                MethodImpl::Native {
                    func: NativeMethod::StrLe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__gt__",
                MethodImpl::Native {
                    func: NativeMethod::StrGt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ge__",
                MethodImpl::Native {
                    func: NativeMethod::StrGe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__eq__",
                MethodImpl::Native {
                    func: NativeMethod::StrEq,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ne__",
                MethodImpl::Native {
                    func: NativeMethod::StrNe,
                    arity: Arity::Exact(1),
                },
            ),
            // 일반 메서드
            (
                "upper",
                MethodImpl::Native {
                    func: NativeMethod::StrUpper,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "lower",
                MethodImpl::Native {
                    func: NativeMethod::StrLower,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "strip",
                MethodImpl::Native {
                    func: NativeMethod::StrStrip,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "split",
                MethodImpl::Native {
                    func: NativeMethod::StrSplit,
                    arity: Arity::Range(0, 1),
                },
            ),
            (
                "join",
                MethodImpl::Native {
                    func: NativeMethod::StrJoin,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "replace",
                MethodImpl::Native {
                    func: NativeMethod::StrReplace,
                    arity: Arity::Exact(2),
                },
            ),
            (
                "startswith",
                MethodImpl::Native {
                    func: NativeMethod::StrStartsWith,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "endswith",
                MethodImpl::Native {
                    func: NativeMethod::StrEndsWith,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "find",
                MethodImpl::Native {
                    func: NativeMethod::StrFind,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "count",
                MethodImpl::Native {
                    func: NativeMethod::StrCount,
                    arity: Arity::Exact(1),
                },
            ),
        ]),
        // TYPE_NONE (3)
        TypeDef::new("NoneType", TypeFlags::IMMUTABLE),
        // TYPE_RANGE (4)
        TypeDef::new("range", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE).with_methods(vec![
            (
                "__iter__",
                MethodImpl::Native {
                    func: NativeMethod::RangeIter,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__has_next__",
                MethodImpl::Native {
                    func: NativeMethod::RangeHasNext,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__next__",
                MethodImpl::Native {
                    func: NativeMethod::RangeNext,
                    arity: Arity::Exact(0),
                },
            ),
        ]),
        // TYPE_LIST (5)
        TypeDef::new("list", TypeFlags::ITERABLE).with_methods(vec![
            (
                "append",
                MethodImpl::Native {
                    func: NativeMethod::ListAppend,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "pop",
                MethodImpl::Native {
                    func: NativeMethod::ListPop,
                    arity: Arity::Range(0, 1),
                },
            ),
            (
                "extend",
                MethodImpl::Native {
                    func: NativeMethod::ListExtend,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "insert",
                MethodImpl::Native {
                    func: NativeMethod::ListInsert,
                    arity: Arity::Exact(2),
                },
            ),
            (
                "remove",
                MethodImpl::Native {
                    func: NativeMethod::ListRemove,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "reverse",
                MethodImpl::Native {
                    func: NativeMethod::ListReverse,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "sort",
                MethodImpl::Native {
                    func: NativeMethod::ListSort,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "clear",
                MethodImpl::Native {
                    func: NativeMethod::ListClear,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "index",
                MethodImpl::Native {
                    func: NativeMethod::ListIndex,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "count",
                MethodImpl::Native {
                    func: NativeMethod::ListCount,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__iter__",
                MethodImpl::Native {
                    func: NativeMethod::ListIter,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__has_next__",
                MethodImpl::Native {
                    func: NativeMethod::ListHasNext,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__next__",
                MethodImpl::Native {
                    func: NativeMethod::ListNext,
                    arity: Arity::Exact(0),
                },
            ),
        ]),
        // TYPE_DICT (6)
        TypeDef::new("dict", TypeFlags::ITERABLE).with_methods(vec![
            (
                "get",
                MethodImpl::Native {
                    func: NativeMethod::DictGet,
                    arity: Arity::Range(1, 2),
                },
            ),
            (
                "keys",
                MethodImpl::Native {
                    func: NativeMethod::DictKeys,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "values",
                MethodImpl::Native {
                    func: NativeMethod::DictValues,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "items",
                MethodImpl::Native {
                    func: NativeMethod::DictItems,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "pop",
                MethodImpl::Native {
                    func: NativeMethod::DictPop,
                    arity: Arity::Range(1, 2),
                },
            ),
            (
                "update",
                MethodImpl::Native {
                    func: NativeMethod::DictUpdate,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "clear",
                MethodImpl::Native {
                    func: NativeMethod::DictClear,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__iter__",
                MethodImpl::Native {
                    func: NativeMethod::DictIter,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__has_next__",
                MethodImpl::Native {
                    func: NativeMethod::DictHasNext,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__next__",
                MethodImpl::Native {
                    func: NativeMethod::DictNext,
                    arity: Arity::Exact(0),
                },
            ),
        ]),
        // TYPE_FLOAT (7)
        TypeDef::new("float", TypeFlags::IMMUTABLE).with_methods(vec![
            (
                "__add__",
                MethodImpl::Native {
                    func: NativeMethod::FloatAdd,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__sub__",
                MethodImpl::Native {
                    func: NativeMethod::FloatSub,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__mul__",
                MethodImpl::Native {
                    func: NativeMethod::FloatMul,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__truediv__",
                MethodImpl::Native {
                    func: NativeMethod::FloatTrueDiv,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__floordiv__",
                MethodImpl::Native {
                    func: NativeMethod::FloatFloorDiv,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__mod__",
                MethodImpl::Native {
                    func: NativeMethod::FloatMod,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__neg__",
                MethodImpl::Native {
                    func: NativeMethod::FloatNeg,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__pos__",
                MethodImpl::Native {
                    func: NativeMethod::FloatPos,
                    arity: Arity::Exact(0),
                },
            ),
            (
                "__lt__",
                MethodImpl::Native {
                    func: NativeMethod::FloatLt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__le__",
                MethodImpl::Native {
                    func: NativeMethod::FloatLe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__gt__",
                MethodImpl::Native {
                    func: NativeMethod::FloatGt,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ge__",
                MethodImpl::Native {
                    func: NativeMethod::FloatGe,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__eq__",
                MethodImpl::Native {
                    func: NativeMethod::FloatEq,
                    arity: Arity::Exact(1),
                },
            ),
            (
                "__ne__",
                MethodImpl::Native {
                    func: NativeMethod::FloatNe,
                    arity: Arity::Exact(1),
                },
            ),
        ]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_def_creation() {
        let mut str_type = TypeDef::new("str", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE);

        str_type.add_method(
            "upper",
            MethodImpl::Native {
                func: NativeMethod::StrUpper,
                arity: Arity::Exact(0),
            },
        );

        assert_eq!(str_type.name, "str");
        assert_eq!(str_type.methods.len(), 1);
        assert!(str_type.methods.contains_key("upper"));
    }

    #[test]
    fn test_arity_check() {
        assert!(Arity::Exact(2).check(2));
        assert!(!Arity::Exact(2).check(3));

        assert!(Arity::Range(1, 3).check(2));
        assert!(!Arity::Range(1, 3).check(4));

        assert!(Arity::Variadic.check(0));
        assert!(Arity::Variadic.check(100));
    }

    #[test]
    fn test_native_method_name() {
        assert_eq!(NativeMethod::StrUpper.name(), "upper");
        assert_eq!(NativeMethod::StrLower.name(), "lower");
        assert_eq!(NativeMethod::RangeIter.name(), "__iter__");
    }

    #[test]
    fn test_builtin_class_name() {
        assert_eq!(BuiltinClassType::Range.name(), "range");
    }

    #[test]
    fn test_type_flags() {
        let flags = TypeFlags::IMMUTABLE | TypeFlags::ITERABLE;
        assert!(flags.contains(TypeFlags::IMMUTABLE));
        assert!(flags.contains(TypeFlags::ITERABLE));
        assert!(!flags.contains(TypeFlags::CALLABLE));
    }
}
