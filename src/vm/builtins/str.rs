//! str builtin type

use super::super::type_def::{Arity, MethodImpl, NativeMethod, TypeDef, TypeFlags};

/// str 타입 정의 등록
pub fn register_type() -> TypeDef {
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
    ])
}
