//! List builtin type

use super::super::type_def::{TypeDef, TypeFlags, MethodImpl, NativeMethod, Arity};

/// List 타입 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("list", TypeFlags::ITERABLE)
        .with_methods(vec![
            ("append", MethodImpl::Native {
                func: NativeMethod::ListAppend,
                arity: Arity::Exact(1),
            }),
            ("pop", MethodImpl::Native {
                func: NativeMethod::ListPop,
                arity: Arity::Range(0, 1),
            }),
            ("extend", MethodImpl::Native {
                func: NativeMethod::ListExtend,
                arity: Arity::Exact(1),
            }),
            ("insert", MethodImpl::Native {
                func: NativeMethod::ListInsert,
                arity: Arity::Exact(2),
            }),
            ("remove", MethodImpl::Native {
                func: NativeMethod::ListRemove,
                arity: Arity::Exact(1),
            }),
            ("reverse", MethodImpl::Native {
                func: NativeMethod::ListReverse,
                arity: Arity::Exact(0),
            }),
            ("sort", MethodImpl::Native {
                func: NativeMethod::ListSort,
                arity: Arity::Exact(0),
            }),
            ("clear", MethodImpl::Native {
                func: NativeMethod::ListClear,
                arity: Arity::Exact(0),
            }),
            ("index", MethodImpl::Native {
                func: NativeMethod::ListIndex,
                arity: Arity::Exact(1),
            }),
            ("count", MethodImpl::Native {
                func: NativeMethod::ListCount,
                arity: Arity::Exact(1),
            }),
            ("__iter__", MethodImpl::Native {
                func: NativeMethod::ListIter,
                arity: Arity::Exact(0),
            }),
            ("__has_next__", MethodImpl::Native {
                func: NativeMethod::ListHasNext,
                arity: Arity::Exact(0),
            }),
            ("__next__", MethodImpl::Native {
                func: NativeMethod::ListNext,
                arity: Arity::Exact(0),
            }),
        ])
}


