use super::bytecode::{
    BUILTIN_BOOL_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_LEN_ID, BUILTIN_PRINT_ID,
    BUILTIN_RANGE_ID, BUILTIN_STR_ID, ClassDef, Instruction as I, Module, Value,
};
use super::type_def::{BuiltinClassType, TYPE_RANGE};
use super::value::{BuiltinInstanceData, Object, ObjectData};
use crate::runtime_io::RuntimeIo;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug)]
pub enum VmErrorKind {
    TypeError(&'static str),
    ZeroDivision,
    ArityError { expected: usize, got: usize },
    UndefinedGlobal(u16),
    StackUnderflow,
    StackOverflow,
}

#[derive(Debug)]
pub struct VmError {
    pub kind: VmErrorKind,
    pub message: String,
}

pub type VmResult<T> = Result<T, VmError>;

/// VM execution state
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmState {
    Running,
    WaitingForInput,
    Finished,
    Error,
}

fn err(kind: VmErrorKind, message: String) -> VmError {
    VmError { kind, message }
}

#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub ip: usize,
    pub func_id: usize,
    pub ret_stack_size: usize,
    pub locals: Vec<Value>,
}

pub struct Vm {
    pub stack: Vec<Value>,
    pub frames: Vec<Frame>,
    pub max_stack: usize,
    pub max_frames: usize,
    pub state: VmState,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(128),
            frames: Vec::with_capacity(32),
            max_stack: 1024,
            max_frames: 256,
            state: VmState::Running,
        }
    }

    /// Get current VM state
    pub fn get_state(&self) -> VmState {
        self.state.clone()
    }

    /// Reset VM state to Running (e.g., after providing input)
    pub fn resume(&mut self) {
        if self.state == VmState::WaitingForInput {
            self.state = VmState::Running;
        }
    }

    /// Check if VM is waiting for input
    pub fn is_waiting_for_input(&self) -> bool {
        self.state == VmState::WaitingForInput
    }

    /// Check if VM has finished execution
    pub fn is_finished(&self) -> bool {
        self.state == VmState::Finished || self.frames.is_empty()
    }

    pub fn run(&mut self, module: &mut Module) -> VmResult<Option<Value>> {
        let mut stdio = crate::runtime_io::StdIo;
        self.run_with_io(module, &mut stdio)
    }

    /// Execute with an explicit runtime I/O provider.
    pub fn run_with_io<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Option<Value>> {
        if module.functions.is_empty() {
            return Ok(None);
        }
        // Only enter the main function if we haven't started yet
        if self.frames.is_empty() {
            self.enter_func(module, 0, 0)?;
        }
        loop {
            let (func_id, ip, code_len) = {
                let f = match self.frames.last() {
                    Some(f) => f,
                    None => break,
                };
                let func = &module.functions[f.func_id];
                (f.func_id, f.ip, func.code.len())
            };
            if ip >= code_len {
                let ret = self.leave_frame()?;
                if self.frames.is_empty() {
                    self.state = VmState::Finished;
                    return Ok(ret);
                }
                if let Some(v) = ret {
                    self.push(v)?;
                }
                continue;
            }
            let ins = &module.functions[func_id].code[ip];
            if let Some(f) = self.frames.last_mut() {
                f.ip = ip + 1;
            }
            match ins {
                I::ConstI64(i) => {
                    self.push(Value::Int(*i))?;
                }
                I::ConstStr(i) => {
                    let s = module.string_pool[*i as usize].clone();
                    // String을 Object로 생성 (통일된 타입 시스템)
                    self.push(self.make_string(s))?;
                }
                I::LoadConst(i) => {
                    let v = module.consts[*i as usize].clone();
                    self.push(v)?;
                }
                I::True => {
                    self.push(Value::Bool(true))?;
                }
                I::False => {
                    self.push(Value::Bool(false))?;
                }
                I::None => {
                    self.push(Value::None)?;
                }
                I::LoadLocal(ix) => {
                    let v = self.get_local(*ix)?;
                    self.push(v)?;
                }
                I::StoreLocal(ix) => {
                    let v = self.pop()?;
                    self.set_local(*ix, v)?;
                }
                I::LoadGlobal(ix) => {
                    let v = module
                        .globals
                        .get(*ix as usize)
                        .and_then(|o| o.clone())
                        .ok_or_else(|| {
                            err(
                                VmErrorKind::UndefinedGlobal(*ix),
                                format!("undefined global {}", *ix),
                            )
                        })?;
                    self.push(v)?;
                }
                I::StoreGlobal(ix) => {
                    let v = self.pop()?;
                    let slot = module.globals.get_mut(*ix as usize).ok_or_else(|| {
                        err(
                            VmErrorKind::UndefinedGlobal(*ix),
                            format!("invalid global index {}", *ix),
                        )
                    })?;
                    *slot = Some(v);
                }
                I::Add => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int + Int
                        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(*y)),
                        // String + String (Object 기반)
                        _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                            let s1 = self.expect_string(&a)?;
                            let s2 = self.expect_string(&b)?;
                            self.make_string(s1.to_string() + s2)
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("add"),
                                "unsupported types for addition".into(),
                            ));
                        }
                    };
                    self.push(result)?;
                }
                I::Sub => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a.wrapping_sub(b)))?;
                }
                I::Mul => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int * Int
                        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(*y)),
                        // String * Int (반복)
                        _ if self.is_string_object(&a) && matches!(b, Value::Int(_)) => {
                            let s = self.expect_string(&a)?;
                            if let Value::Int(n) = b {
                                if n < 0 {
                                    self.make_string(String::new())
                                } else {
                                    self.make_string(s.repeat(n as usize))
                                }
                            } else {
                                unreachable!()
                            }
                        }
                        // Int * String (반복, 순서 바꿈)
                        _ if matches!(a, Value::Int(_)) && self.is_string_object(&b) => {
                            let s = self.expect_string(&b)?;
                            if let Value::Int(n) = a {
                                if n < 0 {
                                    self.make_string(String::new())
                                } else {
                                    self.make_string(s.repeat(n as usize))
                                }
                            } else {
                                unreachable!()
                            }
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("multiply"),
                                "unsupported types for multiplication".into(),
                            ));
                        }
                    };
                    self.push(result)?;
                }
                I::Div => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer division by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a.wrapping_div(b)))?;
                }
                I::Mod => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer modulo by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a.wrapping_rem(b)))?;
                }
                I::Neg => {
                    let a = self.pop_int()?;
                    self.push(Value::Int(a.wrapping_neg()))?;
                }
                I::Pos => {
                    let a = self.pop_int()?;
                    self.push(Value::Int(a))?;
                }
                I::Eq => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(Value::Bool(eq_vals(&a, &b)))?;
                }
                I::Ne => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    self.push(Value::Bool(!eq_vals(&a, &b)))?;
                }
                I::Lt => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int < Int
                        (Value::Int(x), Value::Int(y)) => x < y,
                        // String < String (사전순)
                        _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                            let s1 = self.expect_string(&a)?;
                            let s2 = self.expect_string(&b)?;
                            s1 < s2
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("compare"),
                                "cannot compare different types".into(),
                            ));
                        }
                    };
                    self.push(Value::Bool(result))?;
                }
                I::Le => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int <= Int
                        (Value::Int(x), Value::Int(y)) => x <= y,
                        // String <= String (사전순)
                        _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                            let s1 = self.expect_string(&a)?;
                            let s2 = self.expect_string(&b)?;
                            s1 <= s2
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("compare"),
                                "cannot compare different types".into(),
                            ));
                        }
                    };
                    self.push(Value::Bool(result))?;
                }
                I::Gt => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int > Int
                        (Value::Int(x), Value::Int(y)) => x > y,
                        // String > String (사전순)
                        _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                            let s1 = self.expect_string(&a)?;
                            let s2 = self.expect_string(&b)?;
                            s1 > s2
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("compare"),
                                "cannot compare different types".into(),
                            ));
                        }
                    };
                    self.push(Value::Bool(result))?;
                }
                I::Ge => {
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (&a, &b) {
                        // Int >= Int
                        (Value::Int(x), Value::Int(y)) => x >= y,
                        // String >= String (사전순)
                        _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                            let s1 = self.expect_string(&a)?;
                            let s2 = self.expect_string(&b)?;
                            s1 >= s2
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("compare"),
                                "cannot compare different types".into(),
                            ));
                        }
                    };
                    self.push(Value::Bool(result))?;
                }
                I::Not => {
                    let a = self.pop_bool()?;
                    self.push(Value::Bool(!a))?;
                }
                I::Jump(off) => {
                    self.add_ip_rel(*off);
                }
                I::JumpIfFalse(off) => {
                    let c = self.pop_bool()?;
                    if !c {
                        self.add_ip_rel(*off);
                    }
                }
                I::JumpIfTrue(off) => {
                    let c = self.pop_bool()?;
                    if c {
                        self.add_ip_rel(*off);
                    }
                }
                I::Call(fid, argc) => {
                    let argc = *argc as usize;
                    self.enter_func(module, *fid as usize, argc)?;
                }
                I::CallBuiltin(bid, argc) => {
                    let argc = *argc as usize;
                    let bid = *bid;
                    match bid {
                        BUILTIN_PRINT_ID => {
                            if argc != 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!(
                                        "print() takes 1 positional argument but {} given",
                                        argc
                                    ),
                                ));
                            }
                            let v = self.pop()?;
                            io.write_line(&display_value(&v));
                            self.push(Value::None)?;
                        }
                        BUILTIN_INPUT_ID => {
                            if argc > 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!(
                                        "input() takes at most 1 positional argument but {} given",
                                        argc
                                    ),
                                ));
                            }

                            // Get prompt if provided (peek to keep on stack for retry)
                            let prompt_str = if argc == 1 {
                                let prompt = self.stack.last().ok_or_else(|| {
                                    err(VmErrorKind::StackUnderflow, "stack underflow".into())
                                })?;
                                // Object에서 String 프롬프트 추출
                                if self.is_string_object(prompt) {
                                    Some(self.expect_string(prompt)?)
                                } else {
                                    return Err(err(
                                        VmErrorKind::TypeError("input"),
                                        "prompt must be a string".to_string(),
                                    ));
                                }
                            } else {
                                None
                            };

                            use crate::runtime_io::ReadResult;
                            // Use read_line_with_prompt which handles prompt deduplication internally
                            match io.read_line_with_prompt(prompt_str) {
                                ReadResult::Ok(line) => {
                                    // Pop the prompt if it exists
                                    if argc == 1 {
                                        self.pop()?;
                                    }
                                    // input 결과를 String Object로 반환
                                    self.push(self.make_string(line.trim().to_string()))?;
                                    self.state = VmState::Running;
                                }
                                ReadResult::WaitingForInput => {
                                    // Mark VM as waiting for input
                                    // Keep the prompt on stack (we peeked, didn't pop)
                                    // Decrement IP so we retry this instruction when input arrives
                                    self.state = VmState::WaitingForInput;
                                    if let Some(f) = self.frames.last_mut() {
                                        f.ip -= 1;
                                    }
                                    return Ok(None);
                                }
                                ReadResult::Error(e) => {
                                    return Err(err(VmErrorKind::TypeError("io"), e));
                                }
                            }
                        }
                        BUILTIN_INT_ID => {
                            if argc != 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!("int() takes 1 positional argument but {} given", argc),
                                ));
                            }
                            let v = self.pop()?;
                            self.push(Value::Int(to_int(&v)))?;
                        }
                        BUILTIN_BOOL_ID => {
                            if argc != 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!(
                                        "bool() takes 1 positional argument but {} given",
                                        argc
                                    ),
                                ));
                            }
                            let v = self.pop()?;
                            self.push(Value::Bool(to_bool(&v)))?;
                        }
                        BUILTIN_STR_ID => {
                            if argc != 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!("str() takes 1 positional argument but {} given", argc),
                                ));
                            }
                            let v = self.pop()?;
                            // Phase 4: display_value 재사용
                            let s = display_value(&v);
                            self.push(self.make_string(s))?;
                        }
                        BUILTIN_LEN_ID => {
                            if argc != 1 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 1,
                                        got: argc,
                                    },
                                    format!("len() takes 1 positional argument but {} given", argc),
                                ));
                            }
                            let v = self.pop()?;
                            // String Object에서 길이 구하기
                            if self.is_string_object(&v) {
                                let s = self.expect_string(&v)?;
                                self.push(Value::Int(s.chars().count() as i64))?;
                            } else {
                                return Err(err(
                                    VmErrorKind::TypeError("len"),
                                    "len() requires a string".into(),
                                ));
                            }
                        }
                        BUILTIN_RANGE_ID => {
                            // range(stop) or range(start, stop) or range(start, stop, step)
                            let mut args = Vec::with_capacity(argc);
                            for _ in 0..argc {
                                args.push(self.pop()?);
                            }
                            args.reverse();
                            
                            let result = self.create_builtin_instance(BuiltinClassType::Range, args)?;
                            self.push(result)?;
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("builtin"),
                                format!("unknown builtin id {}", bid),
                            ));
                        }
                    }
                }
                I::CallValue(argc) => {
                    let argc = *argc as usize;
                    // 인자들 팝
                    let mut args = Vec::with_capacity(argc);
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    // callable 팝
                    let callable = self.pop()?;

                    // callable 타입에 따라 호출
                    match &callable {
                        // Phase 4: 모든 callable이 Object로 통합
                        Value::Object(obj) => match &obj.data {
                            // 사용자 정의 클래스 호출
                            ObjectData::UserClass { class_id, methods } => {
                                // 인스턴스 생성
                                let instance_value = self.make_user_instance(*class_id);

                                // __init__ 메서드가 있으면 호출
                                if let Some(&init_func_id) = methods.get("__init__") {
                                    // __init__(self, *args) 호출
                                    let mut init_args = vec![instance_value.clone()];
                                    init_args.extend(args);
                                    let num_args = init_args.len();

                                    // 인자를 순서대로 스택에 푸시
                                    for arg in init_args {
                                        self.push(arg)?;
                                    }

                                    // __init__ 함수 호출
                                    self.enter_func(module, init_func_id as usize, num_args)?;

                                    // 스택에 instance를 먼저 저장 (return 시 instance가 남도록)
                                    self.stack.insert(
                                        self.frames.last().unwrap().ret_stack_size,
                                        instance_value,
                                    );
                                    continue;
                                } else if !args.is_empty() {
                                    let class_def = &module.classes[*class_id as usize];
                                    return Err(err(
                                        VmErrorKind::ArityError {
                                            expected: 0,
                                            got: args.len(),
                                        },
                                        format!("{}() takes no arguments", class_def.name),
                                    ));
                                }

                                self.push(instance_value)?;
                            }
                            // Builtin 클래스 호출 (range 등)
                            ObjectData::BuiltinClass { class_type } => {
                                let result = self.create_builtin_instance(*class_type, args)?;
                                self.push(result)?;
                            }
                            _ => {
                                return Err(err(
                                    VmErrorKind::TypeError("callable"),
                                    format!("{:?} is not callable", callable),
                                ));
                            }
                        },
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("callable"),
                                format!("{:?} is not callable", callable),
                            ));
                        }
                    }
                }
                I::CallMethod(method_sym, argc) => {
                    let argc = *argc as usize;
                    // 통일된 메서드 호출 핸들러
                    self.handle_call_method(*method_sym, argc, module, io)?;
                }
                I::LoadAttr(attr_sym) => {
                    let obj_value = self.pop()?;
                    let attr_name = &module.symbols[*attr_sym as usize];

                    // Phase 4: 모든 Object에서 속성 로드 가능
                    let value = match &obj_value {
                        Value::Object(obj) => obj.get_attr(attr_name).ok_or_else(|| {
                            err(
                                VmErrorKind::TypeError("attribute"),
                                format!("Object has no attribute '{}'", attr_name),
                            )
                        })?,
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("attribute access"),
                                format!("{:?} has no attributes", obj_value),
                            ));
                        }
                    };

                    self.push(value)?;
                }
                I::StoreAttr(attr_sym) => {
                    let value = self.pop()?;
                    let obj_value = self.pop()?;
                    let attr_name = &module.symbols[*attr_sym as usize];

                    // Phase 4: 모든 Object에 속성 저장 가능
                    match obj_value {
                        Value::Object(obj) => {
                            // Object는 Rc이므로 내부 가변성 사용
                            // set_attr는 &mut를 받지만, Rc::get_mut는 사용 불가
                            // attributes가 RefCell이므로 borrow_mut 사용
                            if let Some(ref attrs) = obj.attributes {
                                attrs.borrow_mut().insert(attr_name.clone(), value);
                            } else {
                                // attributes가 None이면 생성 필요
                                // 하지만 Rc 안의 Object를 수정할 수 없으므로 에러
                                return Err(err(
                                    VmErrorKind::TypeError("attribute assignment"),
                                    "Cannot add attributes to immutable object".into(),
                                ));
                            }
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("attribute assignment"),
                                "Cannot set attributes on non-object".into(),
                            ));
                        }
                    }
                }
                I::Return => {
                    let ret = self.leave_frame()?;
                    if self.frames.is_empty() {
                        self.state = VmState::Finished;
                        return Ok(ret);
                    } else if let Some(v) = ret {
                        self.push(v)?;
                    }
                }
            }
        }
        self.state = VmState::Finished;
        Ok(None)
    }

    fn push(&mut self, v: Value) -> VmResult<()> {
        if self.stack.len() >= self.max_stack {
            return Err(err(VmErrorKind::StackOverflow, "stack overflow".into()));
        }
        self.stack.push(v);
        Ok(())
    }
    fn pop(&mut self) -> VmResult<Value> {
        self.stack
            .pop()
            .ok_or_else(|| err(VmErrorKind::StackUnderflow, "stack underflow".into()))
    }
    fn pop_int(&mut self) -> VmResult<i64> {
        match self.pop()? {
            Value::Int(i) => Ok(i),
            _ => Err(err(VmErrorKind::TypeError("int"), "expected Int".into())),
        }
    }
    fn pop_bool(&mut self) -> VmResult<bool> {
        match self.pop()? {
            Value::Bool(b) => Ok(b),
            Value::Int(i) => Ok(i != 0),
            _ => Err(err(VmErrorKind::TypeError("bool"), "expected Bool".into())),
        }
    }

    fn enter_func(&mut self, module: &Module, func_id: usize, argc: usize) -> VmResult<()> {
        if self.frames.len() >= self.max_frames {
            return Err(err(VmErrorKind::StackOverflow, "frame overflow".into()));
        }
        let locals = {
            // arguments are on stack top in order: arg[n-1]..arg[0]
            // place them into locals[0..argc-1]
            let mut locals = vec![Value::None; module.functions[func_id].num_locals as usize];
            for i in (0..argc).rev() {
                locals[i] = self.pop()?;
            }
            locals
        };
        let frame = Frame {
            ip: 0,
            func_id,
            ret_stack_size: self.stack.len(),
            locals,
        };
        self.frames.push(frame);
        Ok(())
    }

    fn leave_frame(&mut self) -> VmResult<Option<Value>> {
        let ret = self.stack.pop();
        let frame = self.frames.pop().expect("leave_frame with no frame");
        // truncate stack to caller base
        while self.stack.len() > frame.ret_stack_size {
            self.stack.pop();
        }
        Ok(ret)
    }

    fn get_local(&self, ix: u16) -> VmResult<Value> {
        let f = self
            .frames
            .last()
            .ok_or_else(|| err(VmErrorKind::StackUnderflow, "no frame".into()))?;
        let v = f.locals.get(ix as usize).ok_or_else(|| {
            err(
                VmErrorKind::UndefinedGlobal(ix),
                format!("invalid local index {}", ix),
            )
        })?;
        Ok(v.clone())
    }

    fn set_local(&mut self, ix: u16, v: Value) -> VmResult<()> {
        let f = self
            .frames
            .last_mut()
            .ok_or_else(|| err(VmErrorKind::StackUnderflow, "no frame".into()))?;
        let slot = f.locals.get_mut(ix as usize).ok_or_else(|| {
            err(
                VmErrorKind::UndefinedGlobal(ix),
                format!("invalid local index {}", ix),
            )
        })?;
        *slot = v;
        Ok(())
    }

    fn add_ip_rel(&mut self, off: i32) {
        if let Some(f) = self.frames.last_mut() {
            jump_rel(&mut f.ip, off);
        }
    }

    fn current_num_locals(&self, _func_id: usize) -> usize {
        0
    }

    fn create_builtin_instance(&self, bt: BuiltinClassType, args: Vec<Value>) -> VmResult<Value> {
        match bt {
            BuiltinClassType::Range => {
                match args.len() {
                    1 => {
                        let stop = self.expect_int_val(&args[0])?;
                        Ok(self.make_range(0, stop, 1))
                    }
                    2 => {
                        let start = self.expect_int_val(&args[0])?;
                        let stop = self.expect_int_val(&args[1])?;
                        Ok(self.make_range(start, stop, 1))
                    }
                    3 => {
                        let start = self.expect_int_val(&args[0])?;
                        let stop = self.expect_int_val(&args[1])?;
                        let step = self.expect_int_val(&args[2])?;

                        if step == 0 {
                            return Err(err(
                                VmErrorKind::TypeError("range"),
                                "range() step must not be zero".into(),
                            ));
                        }

                        Ok(self.make_range(start, stop, step))
                    }
                    _ => Err(err(
                        VmErrorKind::ArityError {
                            expected: 3,
                            got: args.len(),
                        },
                        "range() takes 1 to 3 arguments".into(),
                    )),
                }
            }
        }
    }

    // Phase 4: call_builtin_method 제거
    // BuiltinObject도 이제 Object이므로 handle_call_method에서 통일 처리

    fn expect_int_val(&self, v: &Value) -> VmResult<i64> {
        match v {
            Value::Int(i) => Ok(*i),
            _ => Err(err(VmErrorKind::TypeError("int"), "expected Int".into())),
        }
    }

    // ========== Object 생성 헬퍼 함수들 ==========

    /// String 객체 생성
    ///
    /// 문자열을 Object로 래핑하여 Value를 생성합니다.
    fn make_string(&self, s: String) -> Value {
        use super::type_def::TYPE_STR;
        Value::Object(Rc::new(Object::new(TYPE_STR, ObjectData::String(s))))
    }

    /// UserClass (타입 객체) 생성
    ///
    /// 사용자 정의 클래스를 Object로 생성합니다.
    fn make_user_class(&self, class_def: &ClassDef) -> Value {
        use super::type_def::TYPE_USER_START;
        let class_id = class_def.methods.get("__class_id__").copied().unwrap_or(0); // 임시: class_id를 어떻게 가져올지 결정 필요

        Value::Object(Rc::new(Object::new(
            TYPE_USER_START + class_id,
            ObjectData::UserClass {
                class_id,
                methods: class_def.methods.clone(),
            },
        )))
    }

    /// UserInstance (인스턴스) 생성
    fn make_user_instance(&self, class_id: u16) -> Value {
        use super::type_def::TYPE_USER_START;
        Value::Object(Rc::new(Object::new_with_attrs(
            TYPE_USER_START + class_id,
            ObjectData::UserInstance { class_id },
        )))
    }

    /// BuiltinClass (range 등) 생성
    fn make_builtin_class(&self, class_type: BuiltinClassType) -> Value {
        Value::Object(Rc::new(Object::new(
            TYPE_RANGE, // 임시로 TYPE_RANGE 사용
            ObjectData::BuiltinClass { class_type },
        )))
    }

    /// Range 인스턴스 생성
    fn make_range(&self, current: i64, stop: i64, step: i64) -> Value {
        Value::Object(Rc::new(Object::new(
            TYPE_RANGE,
            ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::Range,
                data: BuiltinInstanceData::Range {
                    current: RefCell::new(current),
                    stop,
                    step,
                },
            },
        )))
    }

    // ========== 통일된 메서드 조회 시스템 ==========

    /// 값의 타입 ID 가져오기
    ///
    /// 모든 값에 대해 통일된 방식으로 타입 ID를 반환합니다.
    ///
    /// # 반환값
    ///
    /// - `Int` → TYPE_INT (0)
    /// - `Bool` → TYPE_BOOL (1)
    /// - `None` → TYPE_NONE (3)
    /// - `Object` → obj.type_id
    fn get_type_id(&self, value: &Value) -> VmResult<u16> {
        use super::type_def::*;
        match value {
            Value::Int(_) => Ok(TYPE_INT),
            Value::Bool(_) => Ok(TYPE_BOOL),
            Value::None => Ok(TYPE_NONE),
            Value::Object(obj) => Ok(obj.type_id),
        }
    }

    /// 통일된 메서드 조회
    ///
    /// Python처럼 모든 타입의 메서드를 동일한 방식으로 조회합니다.
    ///
    /// # 알고리즘
    ///
    /// 1. 값의 타입 ID 가져오기
    /// 2. Module.types에서 TypeDef 조회
    /// 3. TypeDef.methods에서 메서드 찾기
    ///
    /// # 예시
    ///
    /// ```
    /// // "hello".upper() 호출 시:
    /// // 1. type_id = TYPE_STR (2)
    /// // 2. type_def = Module.types[2] (str 타입)
    /// // 3. method = type_def.methods["upper"] (NativeMethod::StrUpper)
    /// ```
    fn lookup_method(
        &self,
        value: &Value,
        method_name: &str,
        module: &Module,
    ) -> VmResult<super::type_def::MethodImpl> {
        use super::type_def::*;

        // 1. 값의 타입 ID 가져오기
        let type_id = self.get_type_id(value)?;

        // 2. 타입 정의 가져오기
        if (type_id as usize) >= module.types.len() {
            return Err(err(
                VmErrorKind::TypeError("type"),
                format!("invalid type id: {}", type_id),
            ));
        }
        let type_def = &module.types[type_id as usize];

        // 3. 메서드 테이블에서 조회
        type_def.methods.get(method_name).cloned().ok_or_else(|| {
            err(
                VmErrorKind::TypeError("method"),
                format!("'{}' object has no method '{}'", type_def.name, method_name),
            )
        })
    }

    /// CallMethod 명령어 핸들러
    ///
    /// 메서드 호출을 처리합니다. Python의 메서드 디스패치와 유사합니다.
    ///
    /// # 처리 과정
    ///
    /// 1. 인자들 수집
    /// 2. receiver 팝
    /// 3. 메서드 조회 (lookup_method)
    /// 4. Native 메서드면 즉시 실행, UserDefined면 함수 호출
    ///
    /// # 주의
    ///
    /// UserObject와 BuiltinObject는 아직 타입 테이블에 없으므로 별도 처리됩니다.
    fn handle_call_method<IO: crate::runtime_io::RuntimeIo>(
        &mut self,
        method_sym: u16,
        argc: usize,
        module: &Module,
        io: &mut IO,
    ) -> VmResult<()> {
        use super::type_def::*;

        // 1. 인자 수집
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse();

        // 2. receiver 팝
        let receiver = self.pop()?;

        // 3. 메서드 이름 가져오기
        let method_name = &module.symbols[method_sym as usize];

        // Phase 4: UserInstance 메서드는 별도 처리 (타입 테이블이 아닌 클래스 테이블 사용)
        if let Value::Object(obj) = &receiver {
            if let ObjectData::UserInstance { class_id } = &obj.data {
                let class_def = &module.classes[*class_id as usize];

                // 메서드 테이블에서 찾기
                let func_id = class_def.methods.get(method_name).ok_or_else(|| {
                    err(
                        VmErrorKind::TypeError("method"),
                        format!("'{}' has no method '{}'", class_def.name, method_name),
                    )
                })?;

                // self를 첫 번째 인자로 추가
                self.push(receiver.clone())?;
                for arg in args {
                    self.push(arg)?;
                }

                // 함수 호출
                self.enter_func(module, *func_id as usize, argc + 1)?;
                return Ok(());
            }
        }

        // 4. 메서드 조회 (통일된 방식!)
        let method_impl = self.lookup_method(&receiver, method_name, module)?;

        // 5. 메서드 호출
        match method_impl {
            MethodImpl::Native { func, arity } => {
                // Arity 체크
                if !arity.check(args.len()) {
                    return Err(err(
                        VmErrorKind::ArityError {
                            expected: match arity {
                                Arity::Exact(n) => n,
                                _ => 0,
                            },
                            got: args.len(),
                        },
                        format!(
                            "{}.{}() takes {} argument(s) but {} given",
                            self.get_type_name(&receiver, module)?,
                            method_name,
                            arity.description(),
                            args.len()
                        ),
                    ));
                }

                // Native 메서드 실행
                let result = self.call_native_method_dispatch(func, &receiver, args, module, io)?;
                self.push(result)?;
            }
            MethodImpl::UserDefined { func_id } => {
                // self + args를 스택에 푸시
                self.push(receiver)?;
                for arg in args {
                    self.push(arg)?;
                }

                // 함수 호출
                self.enter_func(module, func_id as usize, argc + 1)?;
            }
        }

        Ok(())
    }

    /// 타입 이름 가져오기 (에러 메시지용)
    fn get_type_name(&self, value: &Value, module: &Module) -> VmResult<String> {
        let type_id = self.get_type_id(value)?;
        Ok(module.types[type_id as usize].name.clone())
    }

    /// Native 메서드 디스패처
    ///
    /// NativeMethod ID에 따라 적절한 Rust 함수를 실행합니다.
    ///
    /// # 구현된 메서드
    ///
    /// - **String**: upper, lower, strip, replace, startswith, endswith, find, count
    /// - **Range**: __iter__
    ///
    /// # 미구현 메서드
    ///
    /// - split, join (리스트 타입 필요)
    fn call_native_method_dispatch(
        &self,
        method: super::type_def::NativeMethod,
        receiver: &Value,
        args: Vec<Value>,
        _module: &Module,
        _io: &mut dyn crate::runtime_io::RuntimeIo,
    ) -> VmResult<Value> {
        // native_methods 모듈의 call_native_method 함수를 호출
        super::native_methods::call_native_method(method, receiver, args)
            .map_err(|e| err(VmErrorKind::TypeError("native method"), e.message))
    }

    /// Value에서 String 데이터 추출
    ///
    /// Object의 ObjectData::String에서 문자열 참조를 가져옵니다.
    fn expect_string<'a>(&self, v: &'a Value) -> VmResult<&'a str> {
        match v {
            Value::Object(obj) => {
                use super::value::ObjectData;
                match &obj.data {
                    ObjectData::String(s) => Ok(s),
                    _ => Err(err(
                        VmErrorKind::TypeError("str"),
                        "expected string object".into(),
                    )),
                }
            }
            _ => Err(err(VmErrorKind::TypeError("str"), "expected String".into())),
        }
    }

    /// String 객체인지 확인
    ///
    /// Value가 String Object (type_id == TYPE_STR)인지 체크합니다.
    fn is_string_object(&self, v: &Value) -> bool {
        match v {
            Value::Object(obj) => {
                use super::type_def::TYPE_STR;
                obj.type_id == TYPE_STR
            }
            _ => false,
        }
    }
}

// ========== 타입 변환 유틸리티 ==========

/// Value를 정수로 변환
///
/// Python의 `int()` 변환 규칙을 따릅니다.
fn to_int(v: &Value) -> i64 {
    match v {
        Value::Int(i) => *i,
        Value::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        // String Object → 파싱, 다른 객체는 0
        Value::Object(obj) => {
            use super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => s.parse::<i64>().unwrap_or(0),
                _ => 0,
            }
        }
        Value::None => 0,
    }
}

/// Value를 불리언으로 변환
///
/// Python의 truthy/falsy 규칙을 따릅니다.
///
/// # 규칙
///
/// - `False`, `0`, `None`, 빈 문자열 → false
/// - 나머지 → true
fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        // String Object → 비어있으면 false, 다른 객체는 true
        Value::Object(obj) => {
            use super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => !s.is_empty(),
                _ => true,
            }
        }
        Value::None => false,
    }
}

/// Value 동등성 비교
///
/// Python의 `==` 연산자 의미론을 구현합니다.
///
/// # 규칙
///
/// - 같은 타입: 값 비교
/// - String Object: 내용 비교 (포인터가 다르더라도)
/// - 다른 객체: 포인터 비교 (identity)
fn eq_vals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::None, Value::None) => true,
        // Phase 4: 모든 객체가 Object로 통합
        (Value::Object(x), Value::Object(y)) => {
            // 포인터가 같으면 동일 객체
            if Rc::ptr_eq(x, y) {
                return true;
            }
            // String은 값 비교
            use super::value::ObjectData;
            match (&x.data, &y.data) {
                (ObjectData::String(s1), ObjectData::String(s2)) => s1 == s2,
                // 다른 객체들은 포인터 비교만 (identity)
                _ => false,
            }
        }
        _ => false,
    }
}

/// Value를 출력 가능한 문자열로 변환
///
/// Python의 `print()` 함수가 사용하는 변환입니다.
fn display_value(v: &Value) -> String {
    use super::value::ObjectData;

    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::None => "None".into(),
        // Phase 4: 모든 객체가 Object로 통합
        Value::Object(obj) => match &obj.data {
            ObjectData::String(s) => s.clone(),
            ObjectData::UserClass { class_id, .. } => {
                // class_id로 이름 찾기는 어려우므로 간단하게 표시
                format!("<class {}>", class_id)
            }
            ObjectData::UserInstance { class_id } => {
                format!("<instance of class {}>", class_id)
            }
            ObjectData::BuiltinClass { class_type } => {
                format!("<class '{}'>", class_type.name())
            }
            ObjectData::BuiltinInstance { class_type, .. } => {
                format!("<{} object>", class_type.name())
            }
        },
    }
}

fn jump_rel(ip: &mut usize, off: i32) {
    if off >= 0 {
        *ip = ip.wrapping_add(off as usize);
    } else {
        *ip = ip.wrapping_sub((-off) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::super::bytecode::FunctionCode;
    use super::*;

    fn make_test_module() -> Module {
        // Module::new()를 사용하면 타입 테이블이 자동으로 초기화됨
        Module::new()
    }

    // ========== 스택 연산 테스트 ==========

    #[test]
    fn test_stack_push_pop() {
        let mut vm = Vm::new();
        assert!(vm.push(Value::Int(42)).is_ok());
        assert_eq!(vm.pop().unwrap(), Value::Int(42));
    }

    #[test]
    fn test_stack_underflow() {
        let mut vm = Vm::new();
        let result = vm.pop();
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e.kind, VmErrorKind::StackUnderflow));
        }
    }

    #[test]
    fn test_stack_overflow() {
        let mut vm = Vm::new();
        vm.max_stack = 2;
        assert!(vm.push(Value::Int(1)).is_ok());
        assert!(vm.push(Value::Int(2)).is_ok());
        let result = vm.push(Value::Int(3));
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e.kind, VmErrorKind::StackOverflow));
        }
    }

    // ========== 명령어별 단위 테스트 ==========

    #[test]
    fn test_const_instructions() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42)],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        // Should return 42
        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_arithmetic_add() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(10), I::ConstI64(32), I::Add],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_arithmetic_sub() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(50), I::ConstI64(8), I::Sub],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_arithmetic_mul() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(6), I::ConstI64(7), I::Mul],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_arithmetic_div() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(84), I::ConstI64(2), I::Div],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_arithmetic_mod() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42), I::ConstI64(10), I::Mod],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(2)));
    }

    #[test]
    fn test_arithmetic_neg() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42), I::Neg],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(-42)));
    }

    #[test]
    fn test_comparison_eq() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42), I::ConstI64(42), I::Eq],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Bool(true)));
    }

    #[test]
    fn test_comparison_lt() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(10), I::ConstI64(42), I::Lt],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Bool(true)));
    }

    #[test]
    fn test_logical_not() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::True, I::Not],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Bool(false)));
    }

    #[test]
    fn test_jump() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![
                I::Jump(2), // Skip next 2 instructions
                I::ConstI64(2),
                I::ConstI64(3),
                I::ConstI64(4),
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        // Should return 4 (skipped 2 and 3)
        assert_eq!(result, Some(Value::Int(4)));
    }

    #[test]
    fn test_jump_if_false() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![
                I::False,
                I::JumpIfFalse(2), // Should jump
                I::ConstI64(1),
                I::ConstI64(2),
                I::ConstI64(3),
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        // Should return 3 (skipped 1 and 2)
        assert_eq!(result, Some(Value::Int(3)));
    }

    #[test]
    fn test_local_variables() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 2,
            code: vec![
                I::ConstI64(42),
                I::StoreLocal(0),
                I::ConstI64(100),
                I::StoreLocal(1),
                I::LoadLocal(0),
                I::LoadLocal(1),
                I::Add,
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        // Should return 142
        assert_eq!(result, Some(Value::Int(142)));
    }

    // ========== 함수 호출 테스트 ==========

    #[test]
    fn test_function_call_no_args() {
        let mut module = make_test_module();

        // Function 0: main
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![
                I::Call(1, 0), // Call function 1 with 0 args
            ],
        });

        // Function 1: returns 42
        module.functions.push(FunctionCode {
            name_sym: 1,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42), I::Return],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_function_call_with_args() {
        let mut module = make_test_module();

        // Function 0: main, calls add(10, 32)
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![
                I::ConstI64(10),
                I::ConstI64(32),
                I::Call(1, 2), // Call function 1 with 2 args
            ],
        });

        // Function 1: add(a, b) -> a + b
        module.functions.push(FunctionCode {
            name_sym: 1,
            arity: 2,
            num_locals: 2,
            code: vec![
                I::LoadLocal(0), // a
                I::LoadLocal(1), // b
                I::Add,
                I::Return,
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        assert_eq!(result, Some(Value::Int(42)));
    }

    #[test]
    fn test_recursive_function() {
        let mut module = make_test_module();

        // Function 0: main, calls factorial(5)
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(5), I::Call(1, 1)],
        });

        // Function 1: factorial(n)
        // if n == 0: return 1
        // else: return n * factorial(n-1)
        module.functions.push(FunctionCode {
            name_sym: 1,
            arity: 1,
            num_locals: 1,
            code: vec![
                I::LoadLocal(0), // n
                I::ConstI64(0),
                I::Eq,
                I::JumpIfFalse(2), // if n != 0, jump to else
                I::ConstI64(1),
                I::Return,
                // else:
                I::LoadLocal(0), // n
                I::LoadLocal(0), // n
                I::ConstI64(1),
                I::Sub,        // n - 1
                I::Call(1, 1), // factorial(n-1)
                I::Mul,        // n * factorial(n-1)
                I::Return,
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module).ok().flatten();

        // 5! = 120
        assert_eq!(result, Some(Value::Int(120)));
    }

    #[test]
    fn test_zero_division_error() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![I::ConstI64(42), I::ConstI64(0), I::Div],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e.kind, VmErrorKind::ZeroDivision));
        }
    }

    #[test]
    fn test_type_error() {
        let mut module = make_test_module();
        module.functions.push(FunctionCode {
            name_sym: 0,
            arity: 0,
            num_locals: 0,
            code: vec![
                I::True,
                I::ConstI64(42),
                I::Add, // Can't add bool + int
            ],
        });

        let mut vm = Vm::new();
        let result = vm.run(&mut module);
        assert!(result.is_err());
        if let Err(e) = result {
            assert!(matches!(e.kind, VmErrorKind::TypeError(_)));
        }
    }
}
