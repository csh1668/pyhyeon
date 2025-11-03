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
    // ========== String 메서드들 (10개) ==========
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
    // ========== Range 메서드들 (3개) ==========
    RangeIter,
    RangeHasNext,
    RangeNext,
    // ========== 미래 확장 ==========
    // Int 메서드들:
    //   IntBitLength, IntToBytes, ...
    // List 메서드들:
    //   ListAppend, ListExtend, ListPop, ListReverse, ListSort, ...
    // Dict 메서드들:
    //   DictKeys, DictValues, DictItems, DictGet, ...
}

impl NativeMethod {
    /// 메서드 이름 가져오기
    pub fn name(&self) -> &'static str {
        match self {
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
            Self::RangeIter => "__iter__",
            Self::RangeHasNext => "__has_next__",
            Self::RangeNext => "__next__",
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
    // 미래 확장:
    // List,   // `[1, 2, 3]`
    // Dict,   // `{"a": 1}`
    // Set,    // `{1, 2, 3}`
    // Tuple,  // `(1, 2, 3)`
}

impl BuiltinClassType {
    /// 타입 이름 반환
    pub fn name(&self) -> &'static str {
        match self {
            Self::Range => "range",
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
pub const TYPE_USER_START: u16 = 100;

// ========== 유틸리티 함수 ==========

/// String 객체 생성 헬퍼
///
/// 문자열을 Object로 래핑하여 Value를 생성합니다.
pub fn make_string(s: String) -> crate::vm::bytecode::Value {
    use crate::vm::value::{Object, ObjectData};
    use crate::vm::bytecode::Value;
    use std::rc::Rc;
    
    Value::Object(Rc::new(Object::new(
        TYPE_STR,
        ObjectData::String(s),
    )))
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
