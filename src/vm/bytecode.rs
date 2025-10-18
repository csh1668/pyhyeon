use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Bool(bool),
    String(String),
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Instruction {
    // constants
    ConstI64(i64),
    ConstStr(u32),
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCode {
    pub name_sym: u16,
    pub arity: u8,
    pub num_locals: u16,
    pub code: Vec<Instruction>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Module {
    pub consts: Vec<Value>,
    pub string_pool: Vec<String>,       // string id -> string
    pub globals: Vec<Option<Value>>,    // indexed by symbol id
    pub symbols: Vec<String>,           // symbol id -> name (debug/lookup aid)
    pub functions: Vec<FunctionCode>,
}

pub const BUILTIN_PRINT_ID: u8 = 0;
pub const BUILTIN_INPUT_ID: u8 = 1;
pub const BUILTIN_INT_ID: u8 = 2;
pub const BUILTIN_BOOL_ID: u8 = 3;
