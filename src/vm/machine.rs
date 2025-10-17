#![allow(dead_code)]
#![allow(unused_variables)]

use super::bytecode::{
    BUILTIN_BOOL_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_PRINT_ID, Instruction as I, Module,
    Value,
};
use crate::runtime_io::RuntimeIo;
use std::collections::VecDeque;

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

fn err(kind: VmErrorKind, message: String) -> VmError {
    VmError { kind, message }
}

#[derive(Debug, Clone, Default)]
struct Frame {
    ip: usize,
    func_id: usize,
    ret_stack_size: usize,
    locals: Vec<Value>,
}

pub struct Vm {
    pub stack: Vec<Value>,
    pub frames: Vec<Frame>,
    pub max_stack: usize,
    pub max_frames: usize,
    // When present, capture output from builtins like print instead of writing to stdout
    pub out: Option<String>,
    // Optional queued input lines consumed by input() builtin (web/interactive)
    pub input: VecDeque<String>,
}

impl Vm {
    pub fn new() -> Self {
        Self {
            stack: Vec::with_capacity(128),
            frames: Vec::with_capacity(32),
            max_stack: 1024,
            max_frames: 256,
            out: None,
            input: VecDeque::new(),
        }
    }

    /// Enable capturing of printed output into an internal buffer (used by Web/WASM)
    pub fn enable_capture(&mut self) {
        self.out = Some(String::new());
    }

    /// Take and clear the captured output buffer
    pub fn take_output(&mut self) -> Option<String> {
        self.out.take()
    }

    /// Push a line of input to be consumed by input() builtin
    pub fn push_input_line<S: Into<String>>(&mut self, line: S) {
        self.input.push_back(line.into());
    }

    pub fn run(&mut self, module: &mut Module) -> VmResult<Option<Value>> {
        if module.functions.is_empty() {
            return Ok(None);
        }
        self.enter_func(module, 0, 0)?;
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
                    return Ok(ret);
                }
                if let Some(v) = ret {
                    self.push(v)?;
                }
                continue;
            }
            let ins = module.functions[func_id].code[ip];
            // advance ip
            if let Some(f) = self.frames.last_mut() {
                f.ip = ip + 1;
            }
            match ins {
                I::ConstI64(i) => {
                    self.push(Value::Int(i))?;
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
                    let v = self.get_local(ix)?;
                    self.push(v)?;
                }
                I::StoreLocal(ix) => {
                    let v = self.pop()?;
                    self.set_local(ix, v)?;
                }
                I::LoadGlobal(ix) => {
                    let v = module
                        .globals
                        .get(ix as usize)
                        .and_then(|o| o.clone())
                        .ok_or_else(|| {
                            err(
                                VmErrorKind::UndefinedGlobal(ix),
                                format!("undefined global {}", ix),
                            )
                        })?;
                    self.push(v)?;
                }
                I::StoreGlobal(ix) => {
                    let v = self.pop()?;
                    let slot = module.globals.get_mut(ix as usize).ok_or_else(|| {
                        err(
                            VmErrorKind::UndefinedGlobal(ix),
                            format!("invalid global index {}", ix),
                        )
                    })?;
                    *slot = Some(v);
                }

                I::Add => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a + b))?;
                }
                I::Sub => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a - b))?;
                }
                I::Mul => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a * b))?;
                }
                I::Div => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer division by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a / b))?;
                }
                I::Mod => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer modulo by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a % b))?;
                }
                I::Neg => {
                    let a = self.pop_int()?;
                    self.push(Value::Int(-a))?;
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
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a < b))?;
                }
                I::Le => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a <= b))?;
                }
                I::Gt => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a > b))?;
                }
                I::Ge => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a >= b))?;
                }
                I::Not => {
                    let a = self.pop_bool()?;
                    self.push(Value::Bool(!a))?;
                }

                I::Jump(off) => {
                    self.add_ip_rel(off);
                }
                I::JumpIfFalse(off) => {
                    let c = self.pop_bool()?;
                    if !c {
                        self.add_ip_rel(off);
                    }
                }
                I::JumpIfTrue(off) => {
                    let c = self.pop_bool()?;
                    if c {
                        self.add_ip_rel(off);
                    }
                }

                I::Call(fid, argc) => {
                    let argc = argc as usize;
                    self.enter_func(module, fid as usize, argc)?;
                }
                I::CallBuiltin(bid, argc) => {
                    let argc = argc as usize;
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
                            let s = display_value(&v);
                            if let Some(buf) = self.out.as_mut() {
                                buf.push_str(&s);
                                buf.push('\n');
                            } else {
                                println!("{}", s);
                            }
                            self.push(Value::None)?;
                        }
                        BUILTIN_INPUT_ID => {
                            if argc != 0 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 0,
                                        got: argc,
                                    },
                                    format!(
                                        "input() takes 0 positional arguments but {} given",
                                        argc
                                    ),
                                ));
                            }
                            if let Some(line) = self.input.pop_front() {
                                let parsed = line.trim().parse::<i64>().map_err(|_| {
                                    err(
                                        VmErrorKind::TypeError("parse"),
                                        "input() expects an integer line".into(),
                                    )
                                })?;
                                self.push(Value::Int(parsed))?;
                            } else {
                                // Fallback to native stdin when no queued input is available
                                use std::io::{self, Read};
                                let mut buf = String::new();
                                io::stdin().read_to_string(&mut buf).map_err(|e| {
                                    err(VmErrorKind::TypeError("io"), format!("IO error: {}", e))
                                })?;
                                let line = buf.lines().next().unwrap_or("");
                                let parsed = line.trim().parse::<i64>().map_err(|_| {
                                    err(
                                        VmErrorKind::TypeError("parse"),
                                        "input() expects an integer line".into(),
                                    )
                                })?;
                                self.push(Value::Int(parsed))?;
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
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("builtin"),
                                format!("unknown builtin id {}", bid),
                            ));
                        }
                    }
                }
                I::Return => {
                    let ret = self.leave_frame()?;
                    if self.frames.is_empty() {
                        return Ok(ret);
                    } else if let Some(v) = ret {
                        self.push(v)?;
                    }
                }
            }
        }
        Ok(None)
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
        self.enter_func(module, 0, 0)?;
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
                    return Ok(ret);
                }
                if let Some(v) = ret {
                    self.push(v)?;
                }
                continue;
            }
            let ins = module.functions[func_id].code[ip];
            if let Some(f) = self.frames.last_mut() {
                f.ip = ip + 1;
            }
            match ins {
                I::ConstI64(i) => {
                    self.push(Value::Int(i))?;
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
                    let v = self.get_local(ix)?;
                    self.push(v)?;
                }
                I::StoreLocal(ix) => {
                    let v = self.pop()?;
                    self.set_local(ix, v)?;
                }
                I::LoadGlobal(ix) => {
                    let v = module
                        .globals
                        .get(ix as usize)
                        .and_then(|o| o.clone())
                        .ok_or_else(|| {
                            err(
                                VmErrorKind::UndefinedGlobal(ix),
                                format!("undefined global {}", ix),
                            )
                        })?;
                    self.push(v)?;
                }
                I::StoreGlobal(ix) => {
                    let v = self.pop()?;
                    let slot = module.globals.get_mut(ix as usize).ok_or_else(|| {
                        err(
                            VmErrorKind::UndefinedGlobal(ix),
                            format!("invalid global index {}", ix),
                        )
                    })?;
                    *slot = Some(v);
                }
                I::Add => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a + b))?;
                }
                I::Sub => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a - b))?;
                }
                I::Mul => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Int(a * b))?;
                }
                I::Div => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer division by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a / b))?;
                }
                I::Mod => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    if b == 0 {
                        return Err(err(
                            VmErrorKind::ZeroDivision,
                            "integer modulo by zero".into(),
                        ));
                    }
                    self.push(Value::Int(a % b))?;
                }
                I::Neg => {
                    let a = self.pop_int()?;
                    self.push(Value::Int(-a))?;
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
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a < b))?;
                }
                I::Le => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a <= b))?;
                }
                I::Gt => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a > b))?;
                }
                I::Ge => {
                    let (b, a) = (self.pop_int()?, self.pop_int()?);
                    self.push(Value::Bool(a >= b))?;
                }
                I::Not => {
                    let a = self.pop_bool()?;
                    self.push(Value::Bool(!a))?;
                }
                I::Jump(off) => {
                    self.add_ip_rel(off);
                }
                I::JumpIfFalse(off) => {
                    let c = self.pop_bool()?;
                    if !c {
                        self.add_ip_rel(off);
                    }
                }
                I::JumpIfTrue(off) => {
                    let c = self.pop_bool()?;
                    if c {
                        self.add_ip_rel(off);
                    }
                }
                I::Call(fid, argc) => {
                    let argc = argc as usize;
                    self.enter_func(module, fid as usize, argc)?;
                }
                I::CallBuiltin(bid, argc) => {
                    let argc = argc as usize;
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
                            if argc != 0 {
                                return Err(err(
                                    VmErrorKind::ArityError {
                                        expected: 0,
                                        got: argc,
                                    },
                                    format!(
                                        "input() takes 0 positional arguments but {} given",
                                        argc
                                    ),
                                ));
                            }
                            let line = io
                                .read_line()
                                .map_err(|e| err(VmErrorKind::TypeError("io"), e))?;
                            let parsed = line.trim().parse::<i64>().map_err(|_| {
                                err(
                                    VmErrorKind::TypeError("parse"),
                                    "input() expects an integer line".into(),
                                )
                            })?;
                            self.push(Value::Int(parsed))?;
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
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("builtin"),
                                format!("unknown builtin id {}", bid),
                            ));
                        }
                    }
                }
                I::Return => {
                    let ret = self.leave_frame()?;
                    if self.frames.is_empty() {
                        return Ok(ret);
                    } else if let Some(v) = ret {
                        self.push(v)?;
                    }
                }
            }
        }
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
        let v = *f.locals.get(ix as usize).ok_or_else(|| {
            err(
                VmErrorKind::UndefinedGlobal(ix),
                format!("invalid local index {}", ix),
            )
        })?;
        Ok(v)
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
        Value::None => 0,
    }
}
fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::None => false,
    }
}
fn eq_vals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::None, Value::None) => true,
        _ => false,
    }
}
fn display_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::None => "None".into(),
    }
}

fn jump_rel(ip: &mut usize, off: i32) {
    if off >= 0 {
        *ip = ip.wrapping_add(off as usize);
    } else {
        *ip = ip.wrapping_sub((-off) as usize);
    }
}
