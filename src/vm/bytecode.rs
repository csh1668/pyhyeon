use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Value {
    Int(i64),
    Bool(bool),
    String(String),
    None,

    // 클래스 시스템
    #[serde(skip)]
    BuiltinClass(BuiltinClassType),
    #[serde(skip)]
    BuiltinObject(Rc<RefCell<BuiltinObject>>),
    #[serde(skip)]
    UserClass(Rc<ClassDef>),
    #[serde(skip)]
    UserObject(Rc<RefCell<UserObject>>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::None, Value::None) => true,
            (Value::BuiltinClass(a), Value::BuiltinClass(b)) => a == b,
            (Value::UserClass(a), Value::UserClass(b)) => Rc::ptr_eq(a, b),
            (Value::BuiltinObject(a), Value::BuiltinObject(b)) => Rc::ptr_eq(a, b),
            (Value::UserObject(a), Value::UserObject(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for Value {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuiltinClassType {
    Range,
    // 나중에: List, Dict, File 등
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuiltinObject {
    pub class_type: BuiltinClassType,
    pub data: BuiltinObjectData,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BuiltinObjectData {
    Range { current: i64, stop: i64, step: i64 },
    // 나중에: List(Vec<Value>), Dict(HashMap<String, Value>) 등
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserObject {
    pub class_id: u16,
    pub attributes: HashMap<String, Value>,
}

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

    // 클래스 시스템
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

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Module {
    pub consts: Vec<Value>,
    pub string_pool: Vec<String>,    // string id -> string
    pub globals: Vec<Option<Value>>, // indexed by symbol id
    pub symbols: Vec<String>,        // symbol id -> name (debug/lookup aid)
    pub functions: Vec<FunctionCode>,
    pub classes: Vec<ClassDef>, // 사용자 정의 클래스 목록
}

pub const BUILTIN_PRINT_ID: u8 = 0;
pub const BUILTIN_INPUT_ID: u8 = 1;
pub const BUILTIN_INT_ID: u8 = 2;
pub const BUILTIN_BOOL_ID: u8 = 3;
pub const BUILTIN_STR_ID: u8 = 4;
pub const BUILTIN_LEN_ID: u8 = 5;
