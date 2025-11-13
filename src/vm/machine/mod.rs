// machine 모듈 - VM 실행 엔진
//
// 이 모듈은 바이트코드를 실행하는 VM을 구현합니다.

use crate::builtins::{BuiltinClassType, TYPE_RANGE, TYPE_STR};
use crate::runtime_io::RuntimeIo;
use crate::vm::bytecode::{ClassDef, Instruction as I, Module, Value};
use crate::vm::utils::{make_builtin_class, make_string, make_user_class, make_user_instance};
use crate::vm::value::{BuiltinInstanceData, Object, ObjectData};
use std::cell::RefCell;
use std::rc::Rc;

// 서브모듈
mod instruction;
mod method_dispatch;

#[cfg(test)]
mod tests;

// ========== 타입 정의 ==========

#[derive(Debug)]
pub enum VmErrorKind {
    TypeError(&'static str),
    ZeroDivision,
    ArityError { expected: usize, got: usize },
    UndefinedGlobal(u16),
    StackUnderflow,
    StackOverflow,
    AssertionError,
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

// ========== 유틸리티 함수 ==========

/// VmError 생성 헬퍼 함수
pub fn err(kind: VmErrorKind, message: String) -> VmError {
    VmError { kind, message }
}

// 유틸리티 함수들은 vm::utils로 이동
use super::utils::{display_value, eq_vals};

/// IP를 상대적으로 점프
fn jump_rel(ip: &mut usize, off: i32) {
    if off >= 0 {
        *ip = ip.wrapping_add(off as usize);
    } else {
        *ip = ip.wrapping_sub((-off) as usize);
    }
}

// ========== VM 구현 ==========

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

    pub fn get_state(&self) -> VmState {
        self.state.clone()
    }

    pub fn resume(&mut self) {
        if self.state == VmState::WaitingForInput {
            self.state = VmState::Running;
        }
    }

    pub fn is_waiting_for_input(&self) -> bool {
        self.state == VmState::WaitingForInput
    }

    pub fn is_finished(&self) -> bool {
        self.state == VmState::Finished || self.frames.is_empty()
    }

    pub fn run(&mut self, module: &mut Module) -> VmResult<Option<Value>> {
        let mut stdio = crate::runtime_io::StdIo;
        self.run_with_io(module, &mut stdio)
    }

    pub fn run_with_io<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Option<Value>> {
        if module.functions.is_empty() {
            return Ok(None);
        }
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

                // 반환값을 스택에 푸시
                if let Some(v) = ret {
                    self.push(v)?;
                }
                continue;
            }
            let ins = &module.functions[func_id].code[ip].clone();
            if let Some(f) = self.frames.last_mut() {
                f.ip = ip + 1;
            }

            use instruction::ExecutionFlow;
            match self.execute_instruction(ins, module, io)? {
                ExecutionFlow::Continue => {}
                ExecutionFlow::WaitingForInput => {
                    self.state = VmState::WaitingForInput;
                    return Ok(None);
                }
                ExecutionFlow::Return(ret) => {
                    self.state = VmState::Finished;
                    return Ok(ret);
                }
            }
        }
        self.state = VmState::Finished;
        Ok(None)
    }

    // ========== 스택 연산 ==========

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

    // ========== 프레임 관리 ==========

    fn enter_func(&mut self, module: &Module, func_id: usize, argc: usize) -> VmResult<()> {
        // 캡처 없는 함수 호출 (호환성)
        self.enter_func_with_captures(module, func_id, argc, vec![])
    }

    fn enter_func_with_captures(
        &mut self,
        module: &Module,
        func_id: usize,
        argc: usize,
        captures: Vec<Value>,
    ) -> VmResult<()> {
        if self.frames.len() >= self.max_frames {
            return Err(err(VmErrorKind::StackOverflow, "frame overflow".into()));
        }
        let num_locals = module.functions[func_id].num_locals as usize;

        // num_locals는 최소한 argc + captures만큼은 있어야 함
        let actual_locals = num_locals.max(argc + captures.len());

        // ret_stack_size는 인자를 팝하기 BEFORE 저장해야 함
        let ret_stack_size = self.stack.len() - argc;

        let locals = {
            let mut locals = vec![Value::None; actual_locals];
            // 파라미터를 locals[0..argc]에 배치
            for i in (0..argc).rev() {
                locals[i] = self.pop()?;
            }
            // 캡처 변수를 locals[argc..]에 배치
            for (i, capture) in captures.into_iter().enumerate() {
                locals[argc + i] = capture;
            }
            locals
        };
        let frame = Frame {
            ip: 0,
            func_id,
            ret_stack_size,
            locals,
        };
        self.frames.push(frame);
        Ok(())
    }

    fn leave_frame(&mut self) -> VmResult<Option<Value>> {
        // 스택에 반환값이 있으면 팝, 없으면 None 반환 (암묵적 return)
        let ret = if self.stack.len() > self.frames.last().unwrap().ret_stack_size {
            self.stack.pop()
        } else {
            None
        };
        let frame = self.frames.pop().expect("leave_frame with no frame");
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
}
