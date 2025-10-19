use super::bytecode::{
    BUILTIN_BOOL_ID, BUILTIN_INPUT_ID, BUILTIN_INT_ID, BUILTIN_PRINT_ID, BUILTIN_STR_ID, BUILTIN_LEN_ID,
    Instruction as I, Module, Value,
};
use crate::runtime_io::RuntimeIo;

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
                            
                            // If there's a prompt argument, output it first
                            if argc == 1 {
                                let prompt = self.pop()?;
                                match prompt {
                                    Value::String(s) => {
                                        io.write(&s);
                                    }
                                    _ => {
                                        return Err(err(
                                            VmErrorKind::TypeError("input"),
                                            "prompt must be a string".to_string(),
                                        ));
                                    }
                                }
                            }
                            
                            use crate::runtime_io::ReadResult;
                            match io.read_line() {
                                ReadResult::Ok(line) => {
                                    // Return the line as a string (trimmed)
                                    self.push(Value::String(line.trim().to_string()))?;
                                    self.state = VmState::Running;
                                }
                                ReadResult::WaitingForInput => {
                                    // Mark VM as waiting for input and return
                                    self.state = VmState::WaitingForInput;
                                    // Don't advance IP - we'll retry this instruction when input arrives
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
                                    format!(
                                        "str() takes 1 positional argument but {} given",
                                        argc
                                    ),
                                ));
                            }
                            let v = self.pop()?;
                            let s = match v {
                                Value::Int(i) => i.to_string(),
                                Value::Bool(b) => (if b { "True" } else { "False" }).to_string(),
                                Value::String(s) => s,
                                Value::None => "None".to_string(),
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
                                    format!(
                                        "len() takes 1 positional argument but {} given",
                                        argc
                                    ),
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
        },
        Value::String(s) => s.parse::<i64>().unwrap_or(0),
        Value::None => 0,
    }
}
fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::String(s) => !s.is_empty(),
        Value::None => false,
    }
}
fn eq_vals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::String(x), Value::String(y)) => x == y,
        (Value::None, Value::None) => true,
        _ => false,
    }
}
fn display_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::String(s) => s.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::bytecode::FunctionCode;

    fn make_test_module() -> Module {
        Module {
            consts: vec![],
            string_pool: vec![],
            globals: vec![],
            symbols: vec![],
            functions: vec![],
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
            code: vec![
                I::ConstI64(42),
            ],
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
            code: vec![
                I::ConstI64(10),
                I::ConstI64(32),
                I::Add,
            ],
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
            code: vec![
                I::ConstI64(50),
                I::ConstI64(8),
                I::Sub,
            ],
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
            code: vec![
                I::ConstI64(6),
                I::ConstI64(7),
                I::Mul,
            ],
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
            code: vec![
                I::ConstI64(84),
                I::ConstI64(2),
                I::Div,
            ],
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
            code: vec![
                I::ConstI64(42),
                I::ConstI64(10),
                I::Mod,
            ],
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
            code: vec![
                I::ConstI64(42),
                I::Neg,
            ],
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
            code: vec![
                I::ConstI64(42),
                I::ConstI64(42),
                I::Eq,
            ],
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
            code: vec![
                I::ConstI64(10),
                I::ConstI64(42),
                I::Lt,
            ],
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
            code: vec![
                I::True,
                I::Not,
            ],
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
            code: vec![
                I::ConstI64(42),
                I::Return,
            ],
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
            code: vec![
                I::ConstI64(5),
                I::Call(1, 1),
            ],
        });
        
        // Function 1: factorial(n)
        // if n == 0: return 1
        // else: return n * factorial(n-1)
        module.functions.push(FunctionCode {
            name_sym: 1,
            arity: 1,
            num_locals: 1,
            code: vec![
                I::LoadLocal(0),     // n
                I::ConstI64(0),
                I::Eq,
                I::JumpIfFalse(2),   // if n != 0, jump to else
                I::ConstI64(1),
                I::Return,
                // else:
                I::LoadLocal(0),     // n
                I::LoadLocal(0),     // n
                I::ConstI64(1),
                I::Sub,              // n - 1
                I::Call(1, 1),       // factorial(n-1)
                I::Mul,              // n * factorial(n-1)
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
            code: vec![
                I::ConstI64(42),
                I::ConstI64(0),
                I::Div,
            ],
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

