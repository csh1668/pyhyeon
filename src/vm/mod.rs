#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod bytecode;
pub mod compiler;
pub mod machine;

pub mod native_methods;
pub mod type_def;
pub mod value;

pub use bytecode::{
    BUILTIN_BOOL_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_PRINT_ID, FunctionCode, Instruction,
    Module, Value,
};
pub use compiler::Compiler;
pub use machine::Vm;

pub use native_methods::{NativeError, NativeResult, call_native_method};
pub use type_def::{Arity, MethodImpl, NativeMethod, TypeDef, TypeFlags};
pub use value::{BuiltinInstanceData, Object, ObjectData};
