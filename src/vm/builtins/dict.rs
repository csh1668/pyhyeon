//! Dict builtin type

use super::super::type_def::{TypeDef, TypeFlags, MethodImpl, NativeMethod, Arity};

/// Dict 타입 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("dict", TypeFlags::ITERABLE)
        .with_methods(vec![
            ("get", MethodImpl::Native {
                func: NativeMethod::DictGet,
                arity: Arity::Range(1, 2),
            }),
            ("keys", MethodImpl::Native {
                func: NativeMethod::DictKeys,
                arity: Arity::Exact(0),
            }),
            ("values", MethodImpl::Native {
                func: NativeMethod::DictValues,
                arity: Arity::Exact(0),
            }),
            ("items", MethodImpl::Native {
                func: NativeMethod::DictItems,
                arity: Arity::Exact(0),
            }),
            ("pop", MethodImpl::Native {
                func: NativeMethod::DictPop,
                arity: Arity::Range(1, 2),
            }),
            ("update", MethodImpl::Native {
                func: NativeMethod::DictUpdate,
                arity: Arity::Exact(1),
            }),
            ("clear", MethodImpl::Native {
                func: NativeMethod::DictClear,
                arity: Arity::Exact(0),
            }),
            ("__iter__", MethodImpl::Native {
                func: NativeMethod::DictIter,
                arity: Arity::Exact(0),
            }),
            ("__has_next__", MethodImpl::Native {
                func: NativeMethod::DictHasNext,
                arity: Arity::Exact(0),
            }),
            ("__next__", MethodImpl::Native {
                func: NativeMethod::DictNext,
                arity: Arity::Exact(0),
            }),
        ])
}

