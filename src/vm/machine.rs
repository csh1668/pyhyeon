use super::bytecode::{
    BUILTIN_BOOL_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_LEN_ID, BUILTIN_PRINT_ID,
    BUILTIN_STR_ID, BuiltinClassType, BuiltinObject, BuiltinObjectData, ClassDef, Instruction as I,
    Module, UserObject, Value,
};
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
                    self.push(Value::String(s))?;
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
                    // let (b, a) = (self.pop_int()?, self.pop_int()?);
                    // self.push(Value::Int(a.wrapping_add(b)))?;
                    let (b, a) = (self.pop()?, self.pop()?);
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(y)),
                        (Value::String(x), Value::String(y)) => Value::String(x + &y),
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
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(y)),
                        (Value::String(s), Value::Int(n)) | (Value::Int(n), Value::String(s)) => {
                            if n < 0 {
                                Value::String(String::new())
                            } else {
                                Value::String(s.repeat(n as usize))
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
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x < y,
                        (Value::String(x), Value::String(y)) => x < y,
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
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x <= y,
                        (Value::String(x), Value::String(y)) => x <= y,
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
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x > y,
                        (Value::String(x), Value::String(y)) => x > y,
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
                    let result = match (a, b) {
                        (Value::Int(x), Value::Int(y)) => x >= y,
                        (Value::String(x), Value::String(y)) => x >= y,
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
                                match prompt {
                                    Value::String(s) => Some(s.as_str()),
                                    _ => {
                                        return Err(err(
                                            VmErrorKind::TypeError("input"),
                                            "prompt must be a string".to_string(),
                                        ));
                                    }
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
                                    // Push the input result
                                    self.push(Value::String(line.trim().to_string()))?;
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
                            let s = match v {
                                Value::Int(i) => i.to_string(),
                                Value::Bool(b) => (if b { "True" } else { "False" }).to_string(),
                                Value::String(s) => s,
                                Value::None => "None".to_string(),
                                Value::UserClass(c) => format!("<class '{}'>", c.name),
                                Value::UserObject(_) => "<object>".to_string(),
                                Value::BuiltinClass(bt) => format!("<class '{:?}'>", bt),
                                Value::BuiltinObject(_) => "<object>".to_string(),
                            };
                            self.push(Value::String(s))?;
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
                            match v {
                                Value::String(s) => {
                                    self.push(Value::Int(s.chars().count() as i64))?;
                                }
                                _ => {
                                    return Err(err(
                                        VmErrorKind::TypeError("len"),
                                        "len() requires a string".into(),
                                    ));
                                }
                            }
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
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    // callable 팝
                    let callable = self.pop()?;

                    // callable 타입에 따라 호출
                    match callable {
                        Value::UserClass(class_def) => {
                            // 인스턴스 생성
                            let class_id = module
                                .classes
                                .iter()
                                .position(|c| c.name == class_def.name)
                                .ok_or_else(|| {
                                    err(
                                        VmErrorKind::TypeError("class"),
                                        format!("Class '{}' not found", class_def.name),
                                    )
                                })? as u16;

                            let instance = UserObject {
                                class_id,
                                attributes: HashMap::new(),
                            };
                            let instance_value = Value::UserObject(Rc::new(RefCell::new(instance)));

                            // __init__ 메서드가 있으면 호출 (동기적으로)
                            if let Some(&init_func_id) = class_def.methods.get("__init__") {
                                // __init__(self, *args) 호출
                                let mut init_args = vec![instance_value.clone()];
                                init_args.extend(args);
                                let num_args = init_args.len();

                                // 인자를 순서대로 스택에 푸시 (enter_func이 역순으로 팝)
                                for arg in init_args {
                                    self.push(arg)?;
                                }

                                // __init__ 함수 호출 및 즉시 실행
                                self.enter_func(module, init_func_id as usize, num_args)?;

                                // __init__을 완전히 실행 (재귀적으로 run 호출하지 않고 loop에서 실행될 것임)
                                // 하지만 인스턴스를 별도로 스택 아래에 저장해둠
                                // 대신: __init__이 Return하면 그 반환값을 버리고 instance를 푸시
                                //
                                // 더 간단한 방법: 스택에 instance를 먼저 저장
                                self.stack.insert(
                                    self.frames.last().unwrap().ret_stack_size,
                                    instance_value,
                                );
                                continue;
                            } else if !args.is_empty() {
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
                        Value::BuiltinClass(bt) => {
                            let result = self.create_builtin_instance(bt, args)?;
                            self.push(result)?;
                        }
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
                    // 인자들 팝
                    let mut args = Vec::new();
                    for _ in 0..argc {
                        args.push(self.pop()?);
                    }
                    args.reverse();

                    // receiver 팝
                    let receiver = self.pop()?;

                    // 메서드 이름 가져오기
                    let method_name = &module.symbols[*method_sym as usize];

                    // 타입별 dispatch
                    let result = match &receiver {
                        // String 메서드
                        Value::String(s) => self.call_string_method(s, method_name, args)?,
                        // Built-in Object 메서드
                        Value::BuiltinObject(obj_rc) => {
                            let obj = obj_rc.borrow();
                            self.call_builtin_method(&obj, method_name, args)?
                        }
                        // User Object 메서드
                        Value::UserObject(obj_rc) => {
                            let obj = obj_rc.borrow();
                            let class_def = &module.classes[obj.class_id as usize];

                            // 메서드 테이블에서 찾기
                            let func_id = class_def.methods.get(method_name).ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    format!("'{}' has no method '{}'", class_def.name, method_name),
                                )
                            })?;

                            // self를 첫 번째 인자로 추가
                            let mut full_args = vec![receiver.clone()];
                            full_args.extend(args);
                            let num_args = full_args.len();

                            // 인자를 순서대로 스택에 푸시 (enter_func이 역순으로 팝)
                            for arg in full_args {
                                self.push(arg)?;
                            }

                            // 함수 호출
                            drop(obj); // borrow 해제
                            self.enter_func(module, *func_id as usize, num_args)?;
                            continue;
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("method call"),
                                format!("{:?} has no methods", receiver),
                            ));
                        }
                    };

                    self.push(result)?;
                }
                I::LoadAttr(attr_sym) => {
                    let obj = self.pop()?;
                    let attr_name = &module.symbols[*attr_sym as usize];

                    let value = match &obj {
                        Value::UserObject(obj_rc) => {
                            let obj = obj_rc.borrow();
                            obj.attributes.get(attr_name).cloned().ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("attribute"),
                                    format!("Object has no attribute '{}'", attr_name),
                                )
                            })?
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("attribute access"),
                                format!("{:?} has no attributes", obj),
                            ));
                        }
                    };

                    self.push(value)?;
                }
                I::StoreAttr(attr_sym) => {
                    let value = self.pop()?;
                    let obj = self.pop()?;
                    let attr_name = &module.symbols[*attr_sym as usize];

                    match obj {
                        Value::UserObject(obj_rc) => {
                            let mut obj = obj_rc.borrow_mut();
                            obj.attributes.insert(attr_name.clone(), value);
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
                let instance = match args.len() {
                    1 => {
                        let stop = self.expect_int_val(&args[0])?;
                        BuiltinObject {
                            class_type: BuiltinClassType::Range,
                            data: BuiltinObjectData::Range {
                                current: 0,
                                stop,
                                step: 1,
                            },
                        }
                    }
                    2 => {
                        let start = self.expect_int_val(&args[0])?;
                        let stop = self.expect_int_val(&args[1])?;
                        BuiltinObject {
                            class_type: BuiltinClassType::Range,
                            data: BuiltinObjectData::Range {
                                current: start,
                                stop,
                                step: 1,
                            },
                        }
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

                        BuiltinObject {
                            class_type: BuiltinClassType::Range,
                            data: BuiltinObjectData::Range {
                                current: start,
                                stop,
                                step,
                            },
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::ArityError {
                                expected: 3,
                                got: args.len(),
                            },
                            "range() takes 1 to 3 arguments".into(),
                        ));
                    }
                };

                Ok(Value::BuiltinObject(Rc::new(RefCell::new(instance))))
            }
        }
    }

    // String 메서드
    fn call_string_method(&self, s: &str, method: &str, args: Vec<Value>) -> VmResult<Value> {
        match method {
            "upper" => {
                if !args.is_empty() {
                    return Err(err(
                        VmErrorKind::ArityError {
                            expected: 0,
                            got: args.len(),
                        },
                        "upper() takes no arguments".into(),
                    ));
                }
                Ok(Value::String(s.to_uppercase()))
            }
            "lower" => {
                if !args.is_empty() {
                    return Err(err(
                        VmErrorKind::ArityError {
                            expected: 0,
                            got: args.len(),
                        },
                        "lower() takes no arguments".into(),
                    ));
                }
                Ok(Value::String(s.to_lowercase()))
            }
            "strip" => {
                if !args.is_empty() {
                    return Err(err(
                        VmErrorKind::ArityError {
                            expected: 0,
                            got: args.len(),
                        },
                        "strip() takes no arguments".into(),
                    ));
                }
                Ok(Value::String(s.trim().to_string()))
            }
            _ => Err(err(
                VmErrorKind::TypeError("method"),
                format!("str has no method '{}'", method),
            )),
        }
    }

    // Built-in 클래스 메서드
    fn call_builtin_method(
        &self,
        obj: &BuiltinObject,
        method: &str,
        args: Vec<Value>,
    ) -> VmResult<Value> {
        match (&obj.class_type, method) {
            (BuiltinClassType::Range, "__iter__") => {
                if !args.is_empty() {
                    return Err(err(
                        VmErrorKind::ArityError {
                            expected: 0,
                            got: args.len(),
                        },
                        "__iter__() takes no arguments".into(),
                    ));
                }
                // Range는 자기 자신이 iterator
                Ok(Value::BuiltinObject(Rc::new(RefCell::new(obj.clone()))))
            }
            _ => Err(err(
                VmErrorKind::TypeError("method"),
                format!("{:?} has no method '{}'", obj.class_type, method),
            )),
        }
    }

    fn expect_int_val(&self, v: &Value) -> VmResult<i64> {
        match v {
            Value::Int(i) => Ok(*i),
            _ => Err(err(VmErrorKind::TypeError("int"), "expected Int".into())),
        }
    }
}

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
        Value::String(s) => s.parse::<i64>().unwrap_or(0),
        Value::None => 0,
        _ => 0, // 클래스 객체들은 0으로
    }
}
fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::String(s) => !s.is_empty(),
        Value::None => false,
        _ => true, // 객체들은 true
    }
}
fn eq_vals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::None, Value::None) => true,
        (Value::UserClass(x), Value::UserClass(y)) => Rc::ptr_eq(x, y),
        (Value::UserObject(x), Value::UserObject(y)) => Rc::ptr_eq(x, y),
        (Value::BuiltinClass(x), Value::BuiltinClass(y)) => x == y,
        (Value::BuiltinObject(x), Value::BuiltinObject(y)) => Rc::ptr_eq(x, y),
        _ => false,
    }
}
fn display_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
        Value::None => "None".into(),
        Value::UserClass(c) => format!("<class '{}'>", c.name),
        Value::UserObject(o) => {
            let obj = o.borrow();
            format!("<{} object>", obj.class_id)
        }
        Value::BuiltinClass(bt) => format!("<class '{:?}'>", bt),
        Value::BuiltinObject(o) => {
            let obj = o.borrow();
            format!("<{:?} object>", obj.class_type)
        }
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
        Module {
            consts: vec![],
            string_pool: vec![],
            globals: vec![],
            symbols: vec![],
            functions: vec![],
            classes: vec![],
        }
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
