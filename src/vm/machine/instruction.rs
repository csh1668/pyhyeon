use crate::vm::bytecode::{Instruction as I, Module, Value};
use crate::runtime_io::RuntimeIo;
use super::{Vm, VmResult, VmErrorKind, err, eq_vals};
use crate::vm::type_def::BuiltinClassType;
use crate::vm::value::ObjectData;

/// 명령어 실행 결과
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionFlow {
    /// 다음 명령어 계속 실행
    Continue,
    /// 입력 대기 (input() builtin)
    WaitingForInput,
    /// 함수 리턴 (프로그램 종료 가능)
    Return(Option<Value>),
}

impl Vm {
    /// 단일 명령어 실행 (디스패처)
    pub(super) fn execute_instruction<IO: RuntimeIo>(
        &mut self,
        ins: &I,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        match ins {
            // ===== 상수 =====
            I::ConstI64(i) => self.handle_const_i64(*i),
            I::ConstStr(i) => self.handle_const_str(*i, module),
            I::LoadConst(i) => self.handle_load_const(*i, module),
            I::True => self.handle_true(),
            I::False => self.handle_false(),
            I::None => self.handle_none(),

            // ===== 로컬/글로벌 변수 =====
            I::LoadLocal(ix) => self.handle_load_local(*ix),
            I::StoreLocal(ix) => self.handle_store_local(*ix),
            I::LoadGlobal(ix) => self.handle_load_global(*ix, module),
            I::StoreGlobal(ix) => self.handle_store_global(*ix, module),

            // ===== 산술 연산 =====
            I::Add => self.handle_add(),
            I::Sub => self.handle_sub(),
            I::Mul => self.handle_mul(),
            I::Div => self.handle_div(),
            I::Mod => self.handle_mod(),
            I::Neg => self.handle_neg(),
            I::Pos => self.handle_pos(),

            // ===== 비교/논리 연산 =====
            I::Eq => self.handle_eq(),
            I::Ne => self.handle_ne(),
            I::Lt => self.handle_lt(),
            I::Le => self.handle_le(),
            I::Gt => self.handle_gt(),
            I::Ge => self.handle_ge(),
            I::Not => self.handle_not(),

            // ===== 제어 흐름 =====
            I::Jump(off) => self.handle_jump(*off),
            I::JumpIfFalse(off) => self.handle_jump_if_false(*off),
            I::JumpIfTrue(off) => self.handle_jump_if_true(*off),
            I::Call(fid, argc) => self.handle_call(*fid, *argc, module),
            I::CallBuiltin(bid, argc) => self.handle_call_builtin(*bid, *argc, module, io),
            I::CallValue(argc) => self.handle_call_value(*argc, module),
            I::CallMethod(method_sym, argc) => self.handle_call_method_dispatch(*method_sym, *argc, module, io),
            I::Return => self.handle_return(),

            // ===== 속성 접근 =====
            I::LoadAttr(attr_sym) => self.handle_load_attr(*attr_sym, module),
            I::StoreAttr(attr_sym) => self.handle_store_attr(*attr_sym, module),
        }
    }

    // ==================== 상수 핸들러 ====================

    fn handle_const_i64(&mut self, i: i64) -> VmResult<ExecutionFlow> {
        self.push(Value::Int(i))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_const_str(&mut self, i: u32, module: &Module) -> VmResult<ExecutionFlow> {
        let s = module.string_pool[i as usize].clone();
        self.push(self.make_string(s))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_load_const(&mut self, i: u32, module: &Module) -> VmResult<ExecutionFlow> {
        let v = module.consts[i as usize].clone();
        self.push(v)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_true(&mut self) -> VmResult<ExecutionFlow> {
        self.push(Value::Bool(true))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_false(&mut self) -> VmResult<ExecutionFlow> {
        self.push(Value::Bool(false))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_none(&mut self) -> VmResult<ExecutionFlow> {
        self.push(Value::None)?;
        Ok(ExecutionFlow::Continue)
    }

    // ==================== 변수 핸들러 ====================

    fn handle_load_local(&mut self, ix: u16) -> VmResult<ExecutionFlow> {
        let v = self.get_local(ix)?;
        self.push(v)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_store_local(&mut self, ix: u16) -> VmResult<ExecutionFlow> {
        let v = self.pop()?;
        self.set_local(ix, v)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_load_global(&mut self, ix: u16, module: &Module) -> VmResult<ExecutionFlow> {
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
        Ok(ExecutionFlow::Continue)
    }

    fn handle_store_global(&mut self, ix: u16, module: &mut Module) -> VmResult<ExecutionFlow> {
        let v = self.pop()?;
        let slot = module.globals.get_mut(ix as usize).ok_or_else(|| {
            err(
                VmErrorKind::UndefinedGlobal(ix),
                format!("invalid global index {}", ix),
            )
        })?;
        *slot = Some(v);
        Ok(ExecutionFlow::Continue)
    }

    // ==================== 산술 연산 핸들러 ====================

    fn handle_add(&mut self) -> VmResult<ExecutionFlow> {
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
        Ok(ExecutionFlow::Continue)
    }

    fn handle_sub(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop_int()?, self.pop_int()?);
        self.push(Value::Int(a.wrapping_sub(b)))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_mul(&mut self) -> VmResult<ExecutionFlow> {
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
        Ok(ExecutionFlow::Continue)
    }

    fn handle_div(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop_int()?, self.pop_int()?);
        if b == 0 {
            return Err(err(VmErrorKind::ZeroDivision, "division by zero".into()));
        }
        self.push(Value::Int(a / b))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_mod(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop_int()?, self.pop_int()?);
        if b == 0 {
            return Err(err(VmErrorKind::ZeroDivision, "modulo by zero".into()));
        }
        self.push(Value::Int(a % b))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_neg(&mut self) -> VmResult<ExecutionFlow> {
        let a = self.pop_int()?;
        self.push(Value::Int(-a))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_pos(&mut self) -> VmResult<ExecutionFlow> {
        let a = self.pop_int()?;
        self.push(Value::Int(a))?;
        Ok(ExecutionFlow::Continue)
    }

    // ==================== 비교/논리 연산 핸들러 ====================

    fn handle_eq(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        self.push(Value::Bool(eq_vals(&a, &b)))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_ne(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        self.push(Value::Bool(!eq_vals(&a, &b)))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_lt(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        let result = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => *x < *y,
            _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                let s1 = self.expect_string(&a)?;
                let s2 = self.expect_string(&b)?;
                s1 < s2
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("comparison"),
                    "unsupported types for '<'".into(),
                ));
            }
        };
        self.push(Value::Bool(result))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_le(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        let result = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => *x <= *y,
            _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                let s1 = self.expect_string(&a)?;
                let s2 = self.expect_string(&b)?;
                s1 <= s2
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("comparison"),
                    "unsupported types for '<='".into(),
                ));
            }
        };
        self.push(Value::Bool(result))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_gt(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        let result = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => *x > *y,
            _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                let s1 = self.expect_string(&a)?;
                let s2 = self.expect_string(&b)?;
                s1 > s2
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("comparison"),
                    "unsupported types for '>'".into(),
                ));
            }
        };
        self.push(Value::Bool(result))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_ge(&mut self) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);
        let result = match (&a, &b) {
            (Value::Int(x), Value::Int(y)) => *x >= *y,
            _ if self.is_string_object(&a) && self.is_string_object(&b) => {
                let s1 = self.expect_string(&a)?;
                let s2 = self.expect_string(&b)?;
                s1 >= s2
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("comparison"),
                    "unsupported types for '>='".into(),
                ));
            }
        };
        self.push(Value::Bool(result))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_not(&mut self) -> VmResult<ExecutionFlow> {
        let a = self.pop_bool()?;
        self.push(Value::Bool(!a))?;
        Ok(ExecutionFlow::Continue)
    }

    // ==================== 제어 흐름 핸들러 ====================

    fn handle_jump(&mut self, off: i32) -> VmResult<ExecutionFlow> {
        self.add_ip_rel(off);
        Ok(ExecutionFlow::Continue)
    }

    fn handle_jump_if_false(&mut self, off: i32) -> VmResult<ExecutionFlow> {
        let c = self.pop_bool()?;
        if !c {
            self.add_ip_rel(off);
        }
        Ok(ExecutionFlow::Continue)
    }

    fn handle_jump_if_true(&mut self, off: i32) -> VmResult<ExecutionFlow> {
        let c = self.pop_bool()?;
        if c {
            self.add_ip_rel(off);
        }
        Ok(ExecutionFlow::Continue)
    }

    fn handle_call(&mut self, fid: u16, argc: u8, module: &Module) -> VmResult<ExecutionFlow> {
        let argc = argc as usize;
        self.enter_func(module, fid as usize, argc)?;
        Ok(ExecutionFlow::Continue)
    }

    /// Builtin 함수 호출 (print, input, int, bool, str, len, range)
    fn handle_call_builtin<IO: RuntimeIo>(
        &mut self,
        bid: u8,
        argc: u8,
        module: &Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let argc = argc as usize;
        
        // input()은 특별 처리 (입력 대기)
        if bid == crate::vm::builtins::BUILTIN_INPUT_ID {
            return self.handle_builtin_input(argc, io);
        }
        
        // 나머지 builtin은 통합 디스패처 사용
        let mut args = Vec::with_capacity(argc);
        for _ in 0..argc {
            args.push(self.pop()?);
        }
        args.reverse(); // 스택에서 꺼낸 순서를 역순으로
        
        let result = crate::vm::builtins::call_builtin(bid, args, io)?;
        
        self.push(result)?;
        Ok(ExecutionFlow::Continue)
    }

    // ===== input() 특별 핸들러 (입력 대기 처리 필요) =====
    
    fn handle_builtin_input<IO: RuntimeIo>(&mut self, argc: usize, io: &mut IO) -> VmResult<ExecutionFlow> {
        if argc > 1 {
            return Err(err(
                VmErrorKind::ArityError {
                    expected: 1,
                    got: argc,
                },
                format!("input() takes at most 1 positional argument but {} given", argc),
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
                Ok(ExecutionFlow::Continue)
            }
            ReadResult::WaitingForInput => {
                // Decrement IP so we retry this instruction when input arrives
                if let Some(f) = self.frames.last_mut() {
                    f.ip -= 1;
                }
                Ok(ExecutionFlow::WaitingForInput)
            }
            ReadResult::Error(e) => {
                Err(err(VmErrorKind::TypeError("io"), e))
            }
        }
    }

    // ===== CallValue 핸들러 =====

    fn handle_call_value(&mut self, argc: u8, module: &mut Module) -> VmResult<ExecutionFlow> {
        let argc = argc as usize;
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
                        
                        // Note: CallValue의 특수한 경우 - continue로 즉시 다음 명령어로
                        // 이 부분은 run_with_io에서 특별 처리 필요
                        return Ok(ExecutionFlow::Continue);
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
                    use crate::vm::type_def::BuiltinClassType;
                    let result = match class_type {
                        BuiltinClassType::Range => crate::vm::builtins::range::create_range(args)?,
                    };
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
        
        Ok(ExecutionFlow::Continue)
    }

    // ===== CallMethod 핸들러 =====

    fn handle_call_method_dispatch<IO: RuntimeIo>(
        &mut self,
        method_sym: u16,
        argc: u8,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let argc = argc as usize;
        // 통일된 메서드 호출 핸들러
        self.handle_call_method(method_sym, argc, module, io)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_return(&mut self) -> VmResult<ExecutionFlow> {
        let ret = self.leave_frame()?;
        if self.frames.is_empty() {
            Ok(ExecutionFlow::Return(ret))
        } else {
            if let Some(v) = ret {
                self.push(v)?;
            }
            Ok(ExecutionFlow::Continue)
        }
    }

    // ==================== 속성 접근 핸들러 ====================

    fn handle_load_attr(&mut self, attr_sym: u16, module: &Module) -> VmResult<ExecutionFlow> {
        let obj_value = self.pop()?;
        let attr_name = &module.symbols[attr_sym as usize];

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
        Ok(ExecutionFlow::Continue)
    }

    fn handle_store_attr(&mut self, attr_sym: u16, module: &Module) -> VmResult<ExecutionFlow> {
        let value = self.pop()?;
        let obj_value = self.pop()?;
        let attr_name = &module.symbols[attr_sym as usize];

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
        
        Ok(ExecutionFlow::Continue)
    }
}

