use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    // Primitive
    Int(i64),
    Bool(bool),
    None,

    #[serde(skip)]
    Object(Rc<super::value::Object>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::Object(a), Value::Object(b)) => {
                if Rc::ptr_eq(a, b) {
                    return true;
                }
                // String은 값 비교
                // TODO: __eq__ 메서드 구현 필요
                use super::value::ObjectData;
                match (&a.data, &b.data) {
                    (ObjectData::String(s1), ObjectData::String(s2)) => s1 == s2,
                    // 다른 객체들은 포인터 비교만 (identity)
                    _ => false,
                }
            }
            _ => false,
        }
    }
}

impl Eq for Value {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClassDef {
    pub name: String,
    pub methods: HashMap<String, u16>, // method_name → function_id
                                       // 나중에: base_class: Option<u16> (상속)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    // constants
    ConstI64(i64),
    ConstStr(u32),
    LoadConst(u32), // Load constant from consts array
    True,
    False,
    None,

    // locals/globals
    LoadLocal(u16),
    StoreLocal(u16),
    LoadGlobal(u16),
    StoreGlobal(u16),

    // arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Neg,
    Pos,

    // compare/logical
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    Not,

    // control flow
    Jump(i32),
    JumpIfFalse(i32),
    JumpIfTrue(i32),

    // call/return
    Call(u16 /* func_id */, u8 /* argc */),
    CallBuiltin(u8 /* builtin_id */, u8 /* argc */),
    Return,

    /// 값을 callable로 호출 (func가 스택에 있음)
    /// Stack: callable, arg1, arg2, ... → result
    CallValue(u8 /* argc */),

    /// 메서드 호출: receiver.method(args)
    /// Stack: receiver, arg1, arg2, ... → result
    CallMethod(u16 /* method_name_sym */, u8 /* argc */),

    /// Attribute 로드: obj.attr
    /// Stack: object → value
    LoadAttr(u16 /* attr_name_sym */),

    /// Attribute 저장: obj.attr = value
    /// Stack: object, value →
    StoreAttr(u16 /* attr_name_sym */),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCode {
    pub name_sym: u16,
    pub arity: u8,
    pub num_locals: u16,
    pub code: Vec<Instruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Module {
    /// 상수 풀
    pub consts: Vec<Value>,

    /// 문자열 풀 (문자열 리터럴)
    pub string_pool: Vec<String>,

    /// 전역 변수 슬롯
    pub globals: Vec<Option<Value>>,

    /// 심볼 테이블 (변수/함수 이름)
    pub symbols: Vec<String>,

    /// 함수 코드 목록
    pub functions: Vec<FunctionCode>,

    /// 사용자 정의 클래스 목록
    pub classes: Vec<ClassDef>,

    /// 타입 테이블 (builtin + user-defined)
    ///
    /// Python의 타입 시스템을 구현합니다.
    /// 인덱스 0-99는 builtin 타입, 100+는 사용자 정의 타입입니다.
    #[serde(skip)]
    pub types: Vec<super::type_def::TypeDef>,
}

impl Default for Module {
    fn default() -> Self {
        Self::new()
    }
}

impl Module {
    /// 새 모듈 생성 및 builtin 타입 초기화
    ///
    /// 5개의 builtin 타입(int, bool, str, NoneType, range)을 자동으로 초기화합니다.
    ///
    /// # 초기화되는 타입들
    ///
    /// - `types[0]` = int (IMMUTABLE)
    /// - `types[1]` = bool (IMMUTABLE)
    /// - `types[2]` = str (IMMUTABLE | ITERABLE, 메서드 10개)
    /// - `types[3]` = NoneType (IMMUTABLE)
    /// - `types[4]` = range (IMMUTABLE | ITERABLE, 메서드 1개)
    pub fn new() -> Self {
        use super::type_def::*;

        let mut types = Vec::new();

        // === Builtin 타입 초기화 ===

        // TYPE_INT (0): 정수 타입
        types.push(TypeDef::new("int", TypeFlags::IMMUTABLE));

        // TYPE_BOOL (1): 불리언 타입
        // 참고: Python에서 bool은 int의 서브타입이지만 여기서는 독립적
        types.push(TypeDef::new("bool", TypeFlags::IMMUTABLE));

        // TYPE_STR (2): 문자열 타입 (10개 메서드)
        types.push(
            TypeDef::new("str", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE).with_methods(vec![
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
                        arity: Arity::Range(0, 1), // split() 또는 split(sep)
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
                        arity: Arity::Exact(2), // replace(old, new)
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
        );

        // TYPE_NONE (3): None 타입
        types.push(TypeDef::new("NoneType", TypeFlags::IMMUTABLE));

        // TYPE_RANGE (4): range 타입 (iterator)
        types.push(
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
        );

        Module {
            consts: Vec::new(),
            string_pool: Vec::new(),
            globals: Vec::new(),
            symbols: Vec::new(),
            functions: Vec::new(),
            classes: Vec::new(),
            types,
        }
    }
}

pub const BUILTIN_PRINT_ID: u8 = 0;
pub const BUILTIN_INPUT_ID: u8 = 1;
pub const BUILTIN_INT_ID: u8 = 2;
pub const BUILTIN_BOOL_ID: u8 = 3;
pub const BUILTIN_STR_ID: u8 = 4;
pub const BUILTIN_LEN_ID: u8 = 5;
pub const BUILTIN_RANGE_ID: u8 = 6;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::type_def::*;

    #[test]
    fn test_module_type_table_initialization() {
        let module = Module::new();

        // 타입 테이블이 5개 (int, bool, str, NoneType, range) 초기화되어야 함
        assert_eq!(module.types.len(), 5);

        // 각 타입의 이름 확인
        assert_eq!(module.types[TYPE_INT as usize].name, "int");
        assert_eq!(module.types[TYPE_BOOL as usize].name, "bool");
        assert_eq!(module.types[TYPE_STR as usize].name, "str");
        assert_eq!(module.types[TYPE_NONE as usize].name, "NoneType");
        assert_eq!(module.types[TYPE_RANGE as usize].name, "range");
    }

    #[test]
    fn test_str_type_has_methods() {
        let module = Module::new();
        let str_type = &module.types[TYPE_STR as usize];

        // str 타입은 10개의 메서드를 가져야 함
        assert_eq!(str_type.methods.len(), 10);

        // 주요 메서드 확인
        assert!(str_type.methods.contains_key("upper"));
        assert!(str_type.methods.contains_key("lower"));
        assert!(str_type.methods.contains_key("strip"));
        assert!(str_type.methods.contains_key("split"));
        assert!(str_type.methods.contains_key("join"));
        assert!(str_type.methods.contains_key("replace"));
        assert!(str_type.methods.contains_key("startswith"));
        assert!(str_type.methods.contains_key("endswith"));
        assert!(str_type.methods.contains_key("find"));
        assert!(str_type.methods.contains_key("count"));
    }

    #[test]
    fn test_str_method_arity() {
        let module = Module::new();
        let str_type = &module.types[TYPE_STR as usize];

        // upper() - 인자 없음
        if let Some(MethodImpl::Native { arity, .. }) = str_type.methods.get("upper") {
            assert_eq!(*arity, Arity::Exact(0));
        } else {
            panic!("upper method not found or not native");
        }

        // split() - 0 또는 1개 인자
        if let Some(MethodImpl::Native { arity, .. }) = str_type.methods.get("split") {
            assert_eq!(*arity, Arity::Range(0, 1));
        } else {
            panic!("split method not found or not native");
        }

        // replace(old, new) - 정확히 2개 인자
        if let Some(MethodImpl::Native { arity, .. }) = str_type.methods.get("replace") {
            assert_eq!(*arity, Arity::Exact(2));
        } else {
            panic!("replace method not found or not native");
        }
    }

    #[test]
    fn test_range_type_has_iter() {
        let module = Module::new();
        let range_type = &module.types[TYPE_RANGE as usize];

        // range 타입은 __iter__ 메서드를 가져야 함
        assert!(range_type.methods.contains_key("__iter__"));

        if let Some(MethodImpl::Native { func, arity }) = range_type.methods.get("__iter__") {
            assert_eq!(*func, NativeMethod::RangeIter);
            assert_eq!(*arity, Arity::Exact(0));
        } else {
            panic!("__iter__ method not found or not native");
        }
    }

    #[test]
    fn test_type_flags() {
        let module = Module::new();

        // int는 IMMUTABLE
        assert!(
            module.types[TYPE_INT as usize]
                .flags
                .contains(TypeFlags::IMMUTABLE)
        );

        // str은 IMMUTABLE | ITERABLE
        let str_flags = module.types[TYPE_STR as usize].flags;
        assert!(str_flags.contains(TypeFlags::IMMUTABLE));
        assert!(str_flags.contains(TypeFlags::ITERABLE));

        // range는 IMMUTABLE | ITERABLE
        let range_flags = module.types[TYPE_RANGE as usize].flags;
        assert!(range_flags.contains(TypeFlags::IMMUTABLE));
        assert!(range_flags.contains(TypeFlags::ITERABLE));
    }

    #[test]
    fn test_module_default_equals_new() {
        let module1 = Module::new();
        let module2 = Module::default();

        // 두 모듈의 타입 테이블 크기가 같아야 함
        assert_eq!(module1.types.len(), module2.types.len());

        // 타입 이름들이 같아야 함
        for (t1, t2) in module1.types.iter().zip(module2.types.iter()) {
            assert_eq!(t1.name, t2.name);
            assert_eq!(t1.methods.len(), t2.methods.len());
        }
    }
}
