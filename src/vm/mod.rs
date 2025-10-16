#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unused_variables)]

pub mod bytecode;
pub mod compiler;
pub mod machine;

pub use bytecode::{Value, Instruction, FunctionCode, Module, BUILTIN_PRINT_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_BOOL_ID};
pub use compiler::Compiler;
pub use machine::Vm;


