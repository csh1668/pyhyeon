use super::{Vm, VmErrorKind, VmResult, eq_vals, err};
use crate::runtime_io::RuntimeIo;
use crate::vm::bytecode::{Instruction as I, Module, Value};
use crate::vm::type_def::{BuiltinClassType, MethodImpl};
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
            I::ConstF64(f) => self.handle_const_f64(*f),
            I::ConstStr(i) => self.handle_const_str(*i, module),
            I::LoadConst(i) => self.handle_load_const(*i, module),
            I::True => self.handle_true(),
            I::False => self.handle_false(),
            I::None => self.handle_none(),

            // ===== 스택 연산 =====
            I::Pop => self.handle_pop(),

            // ===== 로컬/글로벌 변수 =====
            I::LoadLocal(ix) => self.handle_load_local(*ix),
            I::StoreLocal(ix) => self.handle_store_local(*ix),
            I::LoadGlobal(ix) => self.handle_load_global(*ix, module),
            I::StoreGlobal(ix) => self.handle_store_global(*ix, module),

            // ===== 산술 연산 =====
            I::Add => self.handle_add(module, io),
            I::Sub => self.handle_sub(module, io),
            I::Mul => self.handle_mul(module, io),
            I::Div => self.handle_div(module, io),
            I::TrueDiv => self.handle_truediv(module, io),
            I::Mod => self.handle_mod(module, io),
            I::Neg => self.handle_neg(module, io),
            I::Pos => self.handle_pos(module, io),

            // ===== 비교/논리 연산 =====
            I::Eq => self.handle_eq(module, io),
            I::Ne => self.handle_ne(module, io),
            I::Lt => self.handle_lt(module, io),
            I::Le => self.handle_le(module, io),
            I::Gt => self.handle_gt(module, io),
            I::Ge => self.handle_ge(module, io),
            I::Not => self.handle_not(),

            // ===== 제어 흐름 =====
            I::Jump(off) => self.handle_jump(*off),
            I::JumpIfFalse(off) => self.handle_jump_if_false(*off),
            I::JumpIfTrue(off) => self.handle_jump_if_true(*off),
            I::Call(fid, argc) => self.handle_call(*fid, *argc, module),
            I::CallBuiltin(bid, argc) => self.handle_call_builtin(*bid, *argc, module, io),
            I::CallValue(argc) => self.handle_call_value(*argc, module, io),
            I::CallMethod(method_sym, argc) => {
                self.handle_call_method_dispatch(*method_sym, *argc, module, io)
            }
            I::Return => self.handle_return(),

            // ===== 속성 접근 =====
            I::LoadAttr(attr_sym) => self.handle_load_attr(*attr_sym, module),
            I::StoreAttr(attr_sym) => self.handle_store_attr(*attr_sym, module),

            // ===== 컬렉션 =====
            I::BuildList(count) => self.handle_build_list(*count),
            I::BuildDict(count) => self.handle_build_dict(*count),
            I::LoadIndex => self.handle_load_index(),
            I::StoreIndex => self.handle_store_index(),

            // ===== Lambda/Closure =====
            I::MakeClosure(func_id, num_captures) => {
                self.handle_make_closure(*func_id, *num_captures, module)
            }
        }
    }

    // ==================== 상수 핸들러 ====================

    fn handle_const_i64(&mut self, i: i64) -> VmResult<ExecutionFlow> {
        self.push(Value::Int(i))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_const_f64(&mut self, f: f64) -> VmResult<ExecutionFlow> {
        self.push(Value::Float(f))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_const_str(&mut self, i: u32, module: &Module) -> VmResult<ExecutionFlow> {
        let s = module.string_pool[i as usize].clone();
        self.push(super::super::utils::make_string(s))?;
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

    // ==================== 스택 연산 핸들러 ====================

    fn handle_pop(&mut self) -> VmResult<ExecutionFlow> {
        self.pop()?;
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

    fn handle_add<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int + Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Int(x.wrapping_add(*y)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float + Float
        if let (Value::Float(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(x + y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 3: Int + Float (자동 변환)
        if let (Value::Int(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(*x as f64 + y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 4: Float + Int (자동 변환)
        if let (Value::Float(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Float(x + *y as f64))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 5: String + String
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(super::super::utils::make_string(s1.to_string() + s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __add__ 메서드 조회 및 호출
        match self.lookup_method(&a, "__add__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        // Native 메서드: 즉시 결과 사용
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        // a를 다시 푸시하고, b를 인자로 사용
                        self.push(a)?;
                        self.push(b)?;

                        // __add__ 심볼 찾기
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__add__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__add__ symbol not found".into(),
                                )
                            })? as u16;

                        // CallMethod 핸들러 호출
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("add"),
                format!(
                    "unsupported operand types for +: '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_sub<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int - Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Int(x.wrapping_sub(*y)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float - Float
        if let (Value::Float(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(x - y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 3: Int - Float
        if let (Value::Int(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(*x as f64 - y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 4: Float - Int
        if let (Value::Float(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Float(x - *y as f64))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __sub__ 메서드 조회
        match self.lookup_method(&a, "__sub__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__sub__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__sub__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("sub"),
                format!(
                    "unsupported operand types for -: '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_mul<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int * Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Int(x.wrapping_mul(*y)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: String * Int
        if self.is_string_object(&a)
            && let Value::Int(n) = b
        {
            let s = super::super::utils::expect_string(&a)?;
            let result = if n < 0 {
                String::new()
            } else {
                s.repeat(n as usize)
            };
            self.push(super::super::utils::make_string(result))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 3: Int * String
        if let Value::Int(n) = a
            && self.is_string_object(&b)
        {
            let s = super::super::utils::expect_string(&b)?;
            let result = if n < 0 {
                String::new()
            } else {
                s.repeat(n as usize)
            };
            self.push(super::super::utils::make_string(result))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 4: Float * Float
        if let (Value::Float(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(x * y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 5: Int * Float
        if let (Value::Int(x), Value::Float(y)) = (&a, &b) {
            self.push(Value::Float(*x as f64 * y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 6: Float * Int
        if let (Value::Float(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Float(x * *y as f64))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __mul__ 메서드 조회
        match self.lookup_method(&a, "__mul__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__mul__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__mul__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("mul"),
                format!(
                    "unsupported operand types for *: '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_div<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int // Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            if *y == 0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "integer division by zero".into(),
                ));
            }
            self.push(Value::Int(x / y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float // Float
        if let (Value::Float(x), Value::Float(y)) = (&a, &b) {
            if *y == 0.0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float floor division by zero".into(),
                ));
            }
            self.push(Value::Float((x / y).floor()))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 3: Int // Float
        if let (Value::Int(x), Value::Float(y)) = (&a, &b) {
            if *y == 0.0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float floor division by zero".into(),
                ));
            }
            self.push(Value::Float(((*x as f64) / y).floor()))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 4: Float // Int
        if let (Value::Float(x), Value::Int(y)) = (&a, &b) {
            if *y == 0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float floor division by zero".into(),
                ));
            }
            self.push(Value::Float((x / (*y as f64)).floor()))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __floordiv__ 메서드 조회
        match self.lookup_method(&a, "__floordiv__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__floordiv__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__floordiv__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("floordiv"),
                format!(
                    "unsupported operand types for //: '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_mod<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int % Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            if *y == 0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "integer modulo by zero".into(),
                ));
            }
            self.push(Value::Int(x % y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float % Float
        if let (Value::Float(x), Value::Float(y)) = (&a, &b) {
            if *y == 0.0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float modulo by zero".into(),
                ));
            }
            self.push(Value::Float(x % y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 3: Int % Float
        if let (Value::Int(x), Value::Float(y)) = (&a, &b) {
            if *y == 0.0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float modulo by zero".into(),
                ));
            }
            self.push(Value::Float((*x as f64) % y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 4: Float % Int
        if let (Value::Float(x), Value::Int(y)) = (&a, &b) {
            if *y == 0 {
                return Err(err(
                    VmErrorKind::ZeroDivision,
                    "float modulo by zero".into(),
                ));
            }
            self.push(Value::Float(x % (*y as f64)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __mod__ 메서드 조회
        match self.lookup_method(&a, "__mod__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__mod__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__mod__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("mod"),
                format!(
                    "unsupported operand types for %: '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_truediv<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // True division always returns Float
        let x_f64 = match a {
            Value::Int(x) => x as f64,
            Value::Float(x) => x,
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("truediv"),
                    format!(
                        "unsupported operand types for /: '{}' and '{}'",
                        self.get_type_name(&a, module).unwrap_or_else(|_| "unknown".to_string()),
                        self.get_type_name(&b, module).unwrap_or_else(|_| "unknown".to_string())
                    ),
                ));
            }
        };

        let y_f64 = match b {
            Value::Int(y) => y as f64,
            Value::Float(y) => y,
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("truediv"),
                    format!(
                        "unsupported operand types for /: '{}' and '{}'",
                        self.get_type_name(&a, module).unwrap_or_else(|_| "unknown".to_string()),
                        self.get_type_name(&b, module).unwrap_or_else(|_| "unknown".to_string())
                    ),
                ));
            }
        };

        if y_f64 == 0.0 {
            return Err(err(
                VmErrorKind::ZeroDivision,
                "division by zero".into(),
            ));
        }

        self.push(Value::Float(x_f64 / y_f64))?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_neg<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let a = self.pop()?;

        // Fast path 1: -Int
        if let Value::Int(x) = a {
            self.push(Value::Int(-x))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: -Float
        if let Value::Float(x) = a {
            self.push(Value::Float(-x))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __neg__ 메서드 조회
        match self.lookup_method(&a, "__neg__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__neg__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__neg__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 0, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("neg"),
                format!(
                    "bad operand type for unary -: '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_pos<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let a = self.pop()?;

        // Fast path 1: +Int
        if let Value::Int(x) = a {
            self.push(Value::Int(x))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: +Float
        if let Value::Float(x) = a {
            self.push(Value::Float(x))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __pos__ 메서드 조회
        match self.lookup_method(&a, "__pos__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__pos__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__pos__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 0, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("pos"),
                format!(
                    "bad operand type for unary +: '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    // ==================== 비교/논리 연산 핸들러 ====================

    fn handle_eq<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path: 기본 타입들은 eq_vals 사용
        if matches!(
            (&a, &b),
            (Value::Int(_), Value::Int(_))
                | (Value::Float(_), Value::Float(_))
                | (Value::Int(_), Value::Float(_))
                | (Value::Float(_), Value::Int(_))
                | (Value::Bool(_), Value::Bool(_))
                | (Value::None, Value::None)
        ) {
            self.push(Value::Bool(eq_vals(&a, &b)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // String 비교
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 == s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __eq__ 메서드 조회
        match self.lookup_method(&a, "__eq__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__eq__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__eq__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => {
                // __eq__가 없으면 기본 동작 (객체 identity 비교)
                self.push(Value::Bool(eq_vals(&a, &b)))?;
                Ok(ExecutionFlow::Continue)
            }
        }
    }

    fn handle_ne<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path: 기본 타입들은 eq_vals 사용
        if matches!(
            (&a, &b),
            (Value::Int(_), Value::Int(_))
                | (Value::Float(_), Value::Float(_))
                | (Value::Int(_), Value::Float(_))
                | (Value::Float(_), Value::Int(_))
                | (Value::Bool(_), Value::Bool(_))
                | (Value::None, Value::None)
        ) {
            self.push(Value::Bool(!eq_vals(&a, &b)))?;
            return Ok(ExecutionFlow::Continue);
        }

        // String 비교
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 != s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __ne__ 메서드 조회
        match self.lookup_method(&a, "__ne__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__ne__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__ne__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => {
                // __ne__가 없으면 기본 동작 (객체 identity 비교)
                self.push(Value::Bool(!eq_vals(&a, &b)))?;
                Ok(ExecutionFlow::Continue)
            }
        }
    }

    fn handle_lt<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int < Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Bool(*x < *y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float comparisons (Float-Float, Int-Float, Float-Int)
        match (&a, &b) {
            (Value::Float(x), Value::Float(y)) => {
                self.push(Value::Bool(x < y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Int(x), Value::Float(y)) => {
                self.push(Value::Bool((*x as f64) < *y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Float(x), Value::Int(y)) => {
                self.push(Value::Bool(*x < (*y as f64)))?;
                return Ok(ExecutionFlow::Continue);
            }
            _ => {}
        }

        // Fast path 3: String < String
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 < s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __lt__ 메서드 조회
        match self.lookup_method(&a, "__lt__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__lt__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__lt__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("comparison"),
                format!(
                    "'<' not supported between instances of '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_le<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int <= Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Bool(*x <= *y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float comparisons
        match (&a, &b) {
            (Value::Float(x), Value::Float(y)) => {
                self.push(Value::Bool(x <= y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Int(x), Value::Float(y)) => {
                self.push(Value::Bool((*x as f64) <= *y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Float(x), Value::Int(y)) => {
                self.push(Value::Bool(*x <= (*y as f64)))?;
                return Ok(ExecutionFlow::Continue);
            }
            _ => {}
        }

        // Fast path 3: String <= String
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 <= s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __le__ 메서드 조회
        match self.lookup_method(&a, "__le__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__le__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__le__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("comparison"),
                format!(
                    "'<=' not supported between instances of '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_gt<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int > Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Bool(*x > *y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float comparisons
        match (&a, &b) {
            (Value::Float(x), Value::Float(y)) => {
                self.push(Value::Bool(x > y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Int(x), Value::Float(y)) => {
                self.push(Value::Bool((*x as f64) > *y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Float(x), Value::Int(y)) => {
                self.push(Value::Bool(*x > (*y as f64)))?;
                return Ok(ExecutionFlow::Continue);
            }
            _ => {}
        }

        // Fast path 3: String > String
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 > s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __gt__ 메서드 조회
        match self.lookup_method(&a, "__gt__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__gt__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__gt__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("comparison"),
                format!(
                    "'>' not supported between instances of '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
    }

    fn handle_ge<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
        let (b, a) = (self.pop()?, self.pop()?);

        // Fast path 1: Int >= Int
        if let (Value::Int(x), Value::Int(y)) = (&a, &b) {
            self.push(Value::Bool(*x >= *y))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Fast path 2: Float comparisons
        match (&a, &b) {
            (Value::Float(x), Value::Float(y)) => {
                self.push(Value::Bool(x >= y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Int(x), Value::Float(y)) => {
                self.push(Value::Bool((*x as f64) >= *y))?;
                return Ok(ExecutionFlow::Continue);
            }
            (Value::Float(x), Value::Int(y)) => {
                self.push(Value::Bool(*x >= (*y as f64)))?;
                return Ok(ExecutionFlow::Continue);
            }
            _ => {}
        }

        // Fast path 3: String >= String
        if self.is_string_object(&a) && self.is_string_object(&b) {
            let s1 = super::super::utils::expect_string(&a)?;
            let s2 = super::super::utils::expect_string(&b)?;
            self.push(Value::Bool(s1 >= s2))?;
            return Ok(ExecutionFlow::Continue);
        }

        // Slow path: __ge__ 메서드 조회
        match self.lookup_method(&a, "__ge__", module) {
            Ok(method_impl) => {
                match self.call_method_impl(method_impl, &a, vec![b.clone()], module, io)? {
                    Some(result) => {
                        self.push(result)?;
                        Ok(ExecutionFlow::Continue)
                    }
                    None => {
                        // UserDefined 메서드: 스택 기반 메서드 호출
                        self.push(a)?;
                        self.push(b)?;
                        let method_sym = module
                            .symbols
                            .iter()
                            .position(|s| s == "__ge__")
                            .ok_or_else(|| {
                                err(
                                    VmErrorKind::TypeError("method"),
                                    "__ge__ symbol not found".into(),
                                )
                            })? as u16;
                        self.handle_call_method(method_sym, 1, module, io)?;
                        Ok(ExecutionFlow::Continue)
                    }
                }
            }
            Err(_) => Err(err(
                VmErrorKind::TypeError("comparison"),
                format!(
                    "'>=' not supported between instances of '{}' and '{}'",
                    self.get_type_name(&a, module)
                        .unwrap_or_else(|_| "unknown".to_string()),
                    self.get_type_name(&b, module)
                        .unwrap_or_else(|_| "unknown".to_string())
                ),
            )),
        }
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

    fn handle_builtin_input<IO: RuntimeIo>(
        &mut self,
        argc: usize,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
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
            let prompt = self
                .stack
                .last()
                .ok_or_else(|| err(VmErrorKind::StackUnderflow, "stack underflow".into()))?;
            // Object에서 String 프롬프트 추출
            if self.is_string_object(prompt) {
                Some(super::super::utils::expect_string(prompt)?)
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
                self.push(super::super::utils::make_string(line.trim().to_string()))?;
                Ok(ExecutionFlow::Continue)
            }
            ReadResult::WaitingForInput => {
                // Decrement IP so we retry this instruction when input arrives
                if let Some(f) = self.frames.last_mut() {
                    f.ip -= 1;
                }
                Ok(ExecutionFlow::WaitingForInput)
            }
            ReadResult::Error(e) => Err(err(VmErrorKind::TypeError("io"), e)),
        }
    }

    // ===== CallValue 핸들러 =====

    fn handle_call_value<IO: RuntimeIo>(
        &mut self,
        argc: u8,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<ExecutionFlow> {
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
            Value::Object(obj) => {
                match &obj.data {
                    // 사용자 정의 클래스 호출
                    ObjectData::UserClass { class_id, methods } => {
                        // 인스턴스 생성
                        let instance_value = super::super::utils::make_user_instance(*class_id);

                        // __init__ 메서드가 있으면 호출
                        if let Some(&init_func_id) = methods.get("__init__") {
                            // __init__를 일반 함수처럼 호출
                            // 컴파일러가 __init__의 마지막에 self를 자동으로 반환하도록 생성함
                            // 1. self를 첫 번째 인자로 푸시
                            self.push(instance_value)?;

                            // 2. 나머지 인자들 푸시
                            for arg in args {
                                self.push(arg)?;
                            }

                            // 3. __init__ 함수 호출 (일반 함수 호출과 동일)
                            self.enter_func(module, init_func_id as usize, argc + 1)?;

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
                            BuiltinClassType::Range => {
                                crate::vm::builtins::range::create_range(args)?
                            }
                            BuiltinClassType::List => {
                                // TODO: list() 생성자는 나중에 구현
                                return Err(err(
                                    VmErrorKind::TypeError("list constructor"),
                                    "list() constructor not yet implemented".to_string(),
                                ));
                            }
                            BuiltinClassType::Dict => {
                                // TODO: dict() 생성자는 나중에 구현
                                return Err(err(
                                    VmErrorKind::TypeError("dict constructor"),
                                    "dict() constructor not yet implemented".to_string(),
                                ));
                            }
                        };
                        self.push(result)?;
                    }
                    // User-defined function/lambda 호출 (Closure 지원)
                    ObjectData::UserFunction { func_id, captures } => {
                        // 인자들을 스택에 push
                        for arg in args {
                            self.push(arg)?;
                        }

                        // 캡처 변수와 함께 함수 호출
                        self.enter_func_with_captures(
                            module,
                            *func_id as usize,
                            argc,
                            captures.clone(),
                        )?;
                        return Ok(ExecutionFlow::Continue);
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("callable"),
                            format!("{:?} is not callable", callable),
                        ));
                    }
                }
            }
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
            // Python에서 함수가 명시적 return 없이 끝나면 암묵적으로 None을 반환
            // (__init__의 경우 leave_frame에서 이미 self로 변환됨)
            let value = ret.unwrap_or(Value::None);
            self.push(value)?;
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

    // ==================== 컬렉션 핸들러 ====================

    fn handle_build_list(&mut self, count: u16) -> VmResult<ExecutionFlow> {
        let mut items = Vec::with_capacity(count as usize);
        for _ in 0..count {
            items.push(self.pop()?);
        }
        items.reverse(); // 스택에서 꺼낸 순서를 역순으로

        use crate::vm::type_def::TYPE_LIST;
        use crate::vm::value::{Object, ObjectData};
        use std::cell::RefCell;
        use std::rc::Rc;

        let list_obj = Value::Object(Rc::new(Object::new(
            TYPE_LIST,
            ObjectData::List {
                items: RefCell::new(items),
            },
        )));

        self.push(list_obj)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_build_dict(&mut self, pair_count: u16) -> VmResult<ExecutionFlow> {
        use crate::vm::type_def::TYPE_DICT;
        use crate::vm::value::{DictKey, Object, ObjectData};
        use std::cell::RefCell;
        use std::collections::HashMap;
        use std::rc::Rc;

        let mut map = HashMap::new();
        for _ in 0..pair_count {
            let value = self.pop()?;
            let key = self.pop()?;

            // key를 DictKey로 변환
            let dict_key = match key {
                Value::Int(i) => DictKey::Int(i),
                Value::Bool(b) => DictKey::Bool(b),
                Value::Object(ref obj) => {
                    if let ObjectData::String(ref s) = obj.data {
                        DictKey::String(s.clone())
                    } else {
                        return Err(err(
                            VmErrorKind::TypeError("dict key"),
                            "Dict keys must be int, bool, or str".to_string(),
                        ));
                    }
                }
                _ => {
                    return Err(err(
                        VmErrorKind::TypeError("dict key"),
                        "Dict keys must be int, bool, or str".to_string(),
                    ));
                }
            };

            map.insert(dict_key, value);
        }

        let dict_obj = Value::Object(Rc::new(Object::new(
            TYPE_DICT,
            ObjectData::Dict {
                map: RefCell::new(map),
            },
        )));

        self.push(dict_obj)?;
        Ok(ExecutionFlow::Continue)
    }

    fn handle_load_index(&mut self) -> VmResult<ExecutionFlow> {
        let index = self.pop()?;
        let obj = self.pop()?;

        match obj {
            Value::Object(ref o) => {
                match &o.data {
                    ObjectData::List { items } => {
                        // 인덱스를 정수로 변환
                        let idx = match index {
                            Value::Int(i) => i,
                            _ => {
                                return Err(err(
                                    VmErrorKind::TypeError("list index"),
                                    "List indices must be integers".to_string(),
                                ));
                            }
                        };

                        let items_ref = items.borrow();
                        let len = items_ref.len() as i64;

                        // 음수 인덱스 지원
                        let actual_idx = if idx < 0 {
                            (len + idx) as usize
                        } else {
                            idx as usize
                        };

                        // 범위 체크
                        if actual_idx >= items_ref.len() {
                            return Err(err(
                                VmErrorKind::TypeError("list index"),
                                format!("List index out of range: {}", idx),
                            ));
                        }

                        let value = items_ref[actual_idx].clone();
                        self.push(value)?;
                    }
                    ObjectData::Dict { map } => {
                        // key를 DictKey로 변환
                        use crate::vm::value::DictKey;
                        let dict_key = match index {
                            Value::Int(i) => DictKey::Int(i),
                            Value::Bool(b) => DictKey::Bool(b),
                            Value::Object(ref obj) => {
                                if let ObjectData::String(ref s) = obj.data {
                                    DictKey::String(s.clone())
                                } else {
                                    return Err(err(
                                        VmErrorKind::TypeError("dict key"),
                                        "Dict keys must be int, bool, or str".to_string(),
                                    ));
                                }
                            }
                            _ => {
                                return Err(err(
                                    VmErrorKind::TypeError("dict key"),
                                    "Dict keys must be int, bool, or str".to_string(),
                                ));
                            }
                        };

                        let map_ref = map.borrow();
                        match map_ref.get(&dict_key) {
                            Some(value) => {
                                self.push(value.clone())?;
                            }
                            None => {
                                return Err(err(
                                    VmErrorKind::TypeError("dict key"),
                                    format!("KeyError: {:?}", dict_key),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("indexing"),
                            "Object does not support indexing".to_string(),
                        ));
                    }
                }
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("indexing"),
                    "Only lists and dicts support indexing".to_string(),
                ));
            }
        }

        Ok(ExecutionFlow::Continue)
    }

    fn handle_store_index(&mut self) -> VmResult<ExecutionFlow> {
        let value = self.pop()?;
        let index = self.pop()?;
        let obj = self.pop()?;

        match obj {
            Value::Object(ref o) => {
                match &o.data {
                    ObjectData::List { items } => {
                        // 인덱스를 정수로 변환
                        let idx = match index {
                            Value::Int(i) => i,
                            _ => {
                                return Err(err(
                                    VmErrorKind::TypeError("list index"),
                                    "List indices must be integers".to_string(),
                                ));
                            }
                        };

                        let mut items_mut = items.borrow_mut();
                        let len = items_mut.len() as i64;

                        // 음수 인덱스 지원
                        let actual_idx = if idx < 0 {
                            (len + idx) as usize
                        } else {
                            idx as usize
                        };

                        // 범위 체크
                        if actual_idx >= items_mut.len() {
                            return Err(err(
                                VmErrorKind::TypeError("list index"),
                                format!("List assignment index out of range: {}", idx),
                            ));
                        }

                        items_mut[actual_idx] = value;
                    }
                    ObjectData::Dict { map } => {
                        // key를 DictKey로 변환
                        use crate::vm::value::DictKey;
                        let dict_key = match index {
                            Value::Int(i) => DictKey::Int(i),
                            Value::Bool(b) => DictKey::Bool(b),
                            Value::Object(ref obj) => {
                                if let ObjectData::String(ref s) = obj.data {
                                    DictKey::String(s.clone())
                                } else {
                                    return Err(err(
                                        VmErrorKind::TypeError("dict key"),
                                        "Dict keys must be int, bool, or str".to_string(),
                                    ));
                                }
                            }
                            _ => {
                                return Err(err(
                                    VmErrorKind::TypeError("dict key"),
                                    "Dict keys must be int, bool, or str".to_string(),
                                ));
                            }
                        };

                        let mut map_mut = map.borrow_mut();
                        map_mut.insert(dict_key, value);
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("indexing"),
                            "Object does not support item assignment".to_string(),
                        ));
                    }
                }
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("indexing"),
                    "Only lists and dicts support item assignment".to_string(),
                ));
            }
        }

        Ok(ExecutionFlow::Continue)
    }

    // ==================== Lambda/Closure 핸들러 ====================

    fn handle_make_closure(
        &mut self,
        func_id: u16,
        num_captures: u8,
        _module: &Module,
    ) -> VmResult<ExecutionFlow> {
        use crate::vm::type_def::TYPE_FUNCTION;
        use crate::vm::value::{Object, ObjectData};
        use std::rc::Rc;

        // Pop captured variables from stack (if any)
        let mut captures = Vec::with_capacity(num_captures as usize);
        for _ in 0..num_captures {
            captures.push(self.pop()?);
        }
        captures.reverse(); // 스택에서 역순으로 pop되므로 뒤집기

        // Create UserFunction object
        let func_obj = Value::Object(Rc::new(Object::new(
            TYPE_FUNCTION,
            ObjectData::UserFunction {
                func_id,
                captures,
            },
        )));

        self.push(func_obj)?;
        Ok(ExecutionFlow::Continue)
    }
}
