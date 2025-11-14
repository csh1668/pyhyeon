#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod builtins; // builtin 함수/클래스 통합 모듈
pub mod bytecode;
pub mod compiler;
pub mod disasm; // 디스어셈블러
pub mod jit; // JIT 컴파일러
pub mod machine; // machine/ 디렉토리

pub mod type_def;
pub mod utils;
pub mod value; // 유틸리티 함수

pub use bytecode::{FunctionCode, Instruction, Module, Value};
pub use compiler::Compiler;
pub use machine::{Vm, VmError, VmErrorKind, VmResult, err};

pub use crate::builtins::{
    BuiltinClassType, TYPE_BOOL, TYPE_DICT, TYPE_FLOAT, TYPE_FUNCTION, TYPE_INT, TYPE_LIST,
    TYPE_NONE, TYPE_RANGE, TYPE_STR,
};
pub use type_def::{
    Arity, MethodImpl, NativeMethod, TYPE_USER_START, TypeDef, TypeFlags, init_builtin_types,
    make_string,
};
pub use value::{BuiltinInstanceData, Object, ObjectData};
