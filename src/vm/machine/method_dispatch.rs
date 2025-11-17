use super::super::bytecode::{Module, Value};
use super::super::type_def::{Arity, MethodImpl};
use super::super::utils::expect_string;
use super::super::value::ObjectData;
use super::{Frame, Vm, VmErrorKind, VmResult, err};
use crate::builtins::{TYPE_BOOL, TYPE_FLOAT, TYPE_INT, TYPE_NONE, TYPE_STR};
use crate::runtime_io::RuntimeIo;
use crate::vm::builtins::float;

impl Vm {
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
    pub(super) fn get_type_id(&self, value: &Value) -> VmResult<u16> {
        match value {
            Value::Int(_) => Ok(TYPE_INT),
            Value::Float(_) => Ok(TYPE_FLOAT),
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
    pub(super) fn lookup_method(
        &self,
        value: &Value,
        method_name: &str,
        module: &Module,
    ) -> VmResult<MethodImpl> {
        // UserInstance는 별도 처리 (클래스 테이블 사용)
        if let Value::Object(obj) = value
            && let ObjectData::UserInstance { class_id } = &obj.data
        {
            let class_def = &module.classes[*class_id as usize];

            // 클래스의 메서드 테이블에서 찾기
            let func_id = class_def.methods.get(method_name).ok_or_else(|| {
                err(
                    VmErrorKind::TypeError("method"),
                    format!(
                        "'{}' object has no method '{}'",
                        class_def.name, method_name
                    ),
                )
            })?;

            // UserDefined 메서드로 반환
            return Ok(MethodImpl::UserDefined { func_id: *func_id });
        }

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

    /// 메서드 구현을 실행하는 헬퍼 함수
    ///
    /// lookup_method로 얻은 MethodImpl을 받아서 실행합니다.
    ///
    /// UserDefined 메서드는 연산자 내에서 동기적으로 실행할 수 없으므로
    /// None을 반환합니다. 이 경우 호출자는 스택 기반 실행을 사용해야 합니다.
    pub(super) fn call_method_impl<IO: RuntimeIo>(
        &mut self,
        method_impl: MethodImpl,
        receiver: &Value,
        args: Vec<Value>,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Option<Value>> {
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
                            "method takes {} argument(s) but {} given",
                            arity.description(),
                            args.len()
                        ),
                    ));
                }

                // Native 메서드 실행
                Ok(Some(self.call_native_method_dispatch(
                    func, receiver, args, module, io,
                )?))
            }
            MethodImpl::UserDefined { .. } => {
                // UserDefined 메서드는 동기적 실행 불가
                // None을 반환하여 호출자가 스택 기반 실행을 사용하도록 함
                Ok(None)
            }
        }
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
    pub(super) fn handle_call_method<IO: RuntimeIo>(
        &mut self,
        method_sym: u16,
        argc: usize,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<()> {
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
        if let Value::Object(obj) = &receiver
            && let ObjectData::UserInstance { class_id } = &obj.data
        {
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
    pub(super) fn get_type_name(&self, value: &Value, module: &Module) -> VmResult<String> {
        let type_id = self.get_type_id(value)?;
        Ok(module.types[type_id as usize].name.clone())
    }

    /// Native 메서드 디스패처
    ///
    /// NativeMethod ID에 따라 적절한 Rust 함수를 실행합니다.
    pub(super) fn call_native_method_dispatch<IO: RuntimeIo>(
        &mut self,
        method: super::super::type_def::NativeMethod,
        receiver: &Value,
        args: Vec<Value>,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Value> {
        use super::super::builtins::{dict_methods, filter, int, list_methods, map, range, str_methods};
        use super::super::type_def::NativeMethod as NM;

        // builtins 모듈에서 직접 호출
        match method {
            // Int 매직 메서드들
            // 실제로는 사용되지 않으나, Dynamic Dispatch를 위해 남겨둠
            NM::IntAdd => int::int_add(receiver, args),
            NM::IntSub => int::int_sub(receiver, args),
            NM::IntMul => int::int_mul(receiver, args),
            NM::IntFloorDiv => int::int_floordiv(receiver, args),
            NM::IntTrueDiv => int::int_truediv(receiver, args),
            NM::IntMod => int::int_mod(receiver, args),
            NM::IntNeg => int::int_neg(receiver, args),
            NM::IntPos => int::int_pos(receiver, args),
            NM::IntLt => int::int_lt(receiver, args),
            NM::IntLe => int::int_le(receiver, args),
            NM::IntGt => int::int_gt(receiver, args),
            NM::IntGe => int::int_ge(receiver, args),
            NM::IntEq => int::int_eq(receiver, args),
            NM::IntNe => int::int_ne(receiver, args),

            // Float 매직 메서드들
            NM::FloatAdd => float::float_add(receiver, args),
            NM::FloatSub => float::float_sub(receiver, args),
            NM::FloatMul => float::float_mul(receiver, args),
            NM::FloatTrueDiv => float::float_true_div(receiver, args),
            NM::FloatFloorDiv => float::float_floor_div(receiver, args),
            NM::FloatMod => float::float_mod(receiver, args),
            NM::FloatNeg => float::float_neg(receiver, args),
            NM::FloatPos => float::float_pos(receiver, args),
            NM::FloatLt => float::float_lt(receiver, args),
            NM::FloatLe => float::float_le(receiver, args),
            NM::FloatGt => float::float_gt(receiver, args),
            NM::FloatGe => float::float_ge(receiver, args),
            NM::FloatEq => float::float_eq(receiver, args),
            NM::FloatNe => float::float_ne(receiver, args),

            // String 매직 메서드들
            NM::StrAdd => str_methods::str_add(receiver, args),
            NM::StrMul => str_methods::str_mul(receiver, args),
            NM::StrLt => str_methods::str_lt(receiver, args),
            NM::StrLe => str_methods::str_le(receiver, args),
            NM::StrGt => str_methods::str_gt(receiver, args),
            NM::StrGe => str_methods::str_ge(receiver, args),
            NM::StrEq => str_methods::str_eq(receiver, args),
            NM::StrNe => str_methods::str_ne(receiver, args),

            // String 일반 메서드들
            NM::StrUpper => str_methods::str_upper(receiver, args),
            NM::StrLower => str_methods::str_lower(receiver, args),
            NM::StrStrip => str_methods::str_strip(receiver, args),
            NM::StrSplit => str_methods::str_split(receiver, args),
            NM::StrJoin => str_methods::str_join(receiver, args),
            NM::StrReplace => str_methods::str_replace(receiver, args),
            NM::StrStartsWith => str_methods::str_starts_with(receiver, args),
            NM::StrEndsWith => str_methods::str_ends_with(receiver, args),
            NM::StrFind => str_methods::str_find(receiver, args),
            NM::StrCount => str_methods::str_count(receiver, args),

            // Range 메서드들
            NM::RangeIter => range::range_iter(receiver, args),
            NM::RangeHasNext => range::range_has_next(receiver, args),
            NM::RangeNext => range::range_next(receiver, args),

            // List 메서드들
            NM::ListAppend => list_methods::list_append(receiver, args),
            NM::ListPop => list_methods::list_pop(receiver, args),
            NM::ListExtend => list_methods::list_extend(receiver, args),
            NM::ListInsert => list_methods::list_insert(receiver, args),
            NM::ListRemove => list_methods::list_remove(receiver, args),
            NM::ListReverse => list_methods::list_reverse(receiver, args),
            NM::ListSort => list_methods::list_sort(receiver, args),
            NM::ListClear => list_methods::list_clear(receiver, args),
            NM::ListIndex => list_methods::list_index(receiver, args),
            NM::ListCount => list_methods::list_count(receiver, args),
            NM::ListIter => list_methods::list_iter(receiver, args),
            NM::ListHasNext => list_methods::list_has_next(receiver, args),
            NM::ListNext => list_methods::list_next(receiver, args),

            // Dict 메서드들
            NM::DictGet => dict_methods::dict_get(receiver, args),
            NM::DictKeys => dict_methods::dict_keys(receiver, args),
            NM::DictValues => dict_methods::dict_values(receiver, args),
            NM::DictItems => dict_methods::dict_items(receiver, args),
            NM::DictPop => dict_methods::dict_pop(receiver, args),
            NM::DictUpdate => dict_methods::dict_update(receiver, args),
            NM::DictClear => dict_methods::dict_clear(receiver, args),
            NM::DictIter => dict_methods::dict_iter(receiver, args),
            NM::DictHasNext => dict_methods::dict_has_next(receiver, args),
            NM::DictNext => dict_methods::dict_next(receiver, args),

            // Map Iterator 메서드들
            NM::MapIter => map::map_iter(receiver, args),
            NM::MapHasNext => map::map_has_next(receiver, args, module, self, io),
            NM::MapNext => map::map_next(receiver, args, module, self, io),

            // Filter Iterator 메서드들
            NM::FilterIter => filter::filter_iter(receiver, args),
            NM::FilterHasNext => filter::filter_has_next(receiver, args, module, self, io),
            NM::FilterNext => filter::filter_next(receiver, args, module, self, io),
        }
    }

    /// String 객체인지 확인
    ///
    /// Value가 String Object (type_id == TYPE_STR)인지 체크합니다.
    pub(super) fn is_string_object(&self, v: &Value) -> bool {
        match v {
            Value::Object(obj) => obj.type_id == TYPE_STR,
            _ => false,
        }
    }

    /// 메서드 호출 헬퍼 (builtin 함수에서 사용)
    ///
    /// receiver.method_name(args...) 형태의 호출을 수행합니다.
    pub fn call_method<IO: RuntimeIo>(
        &mut self,
        receiver: &Value,
        method_name: &str,
        args: Vec<Value>,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Value> {
        // 메서드 조회
        let method_impl = self.lookup_method(receiver, method_name, module)?;

        // 메서드 호출
        match self.call_method_impl(method_impl, receiver, args, module, io)? {
            Some(result) => Ok(result),
            None => {
                // UserDefined 메서드는 동기적으로 호출할 수 없음
                // 이 경우는 현재 구조상 발생하지 않아야 함
                Err(err(
                    VmErrorKind::TypeError("method"),
                    "cannot call user-defined method synchronously from builtin".into(),
                ))
            }
        }
    }

    /// 함수 호출 헬퍼 (builtin 함수에서 사용)
    ///
    /// func(args...) 형태의 호출을 수행합니다.
    pub fn call_function<IO: RuntimeIo>(
        &mut self,
        func: &Value,
        args: Vec<Value>,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Value> {
        match func {
            Value::Object(obj) => match &obj.data {
                ObjectData::UserFunction { func_id, captures } => {
                    // 인자들을 스택에 푸시
                    for arg in args.iter() {
                        self.push(arg.clone())?;
                    }

                    // enter_func_with_captures를 사용하여 프레임 생성
                    self.enter_func_with_captures(
                        module,
                        *func_id as usize,
                        args.len(),
                        captures.clone(),
                    )?;

                    // 함수 실행 (Return instruction이 자동으로 leave_frame 호출)
                    let result = self.run_function(module, io)?;

                    Ok(result)
                }
                _ => Err(err(
                    VmErrorKind::TypeError("function"),
                    format!("'{}' object is not callable", super::super::utils::type_name(func)),
                )),
            },
            _ => Err(err(
                VmErrorKind::TypeError("function"),
                format!("'{}' object is not callable", super::super::utils::type_name(func)),
            )),
        }
    }

    /// 함수를 끝까지 실행하고 반환값을 얻음 (동기적 실행)
    fn run_function<IO: RuntimeIo>(
        &mut self,
        module: &mut Module,
        io: &mut IO,
    ) -> VmResult<Value> {
        // 시작 시점의 프레임 개수 저장 (nested 호출 대응)
        let initial_frame_count = self.frames.len();

        loop {
            // 함수가 종료되면 (초기 프레임 개수보다 적어지면) 반환
            if self.frames.len() < initial_frame_count {
                // 함수가 종료됨 - 스택에서 반환값 팝
                return self.pop();
            }

            // 현재 프레임 가져오기
            let frame_idx = self.frames.len() - 1;
            let ip = self.frames[frame_idx].ip;
            let func_id = self.frames[frame_idx].func_id;

            // inst를 복사하여 mutable borrow 문제 해결
            let inst = module.functions[func_id].code[ip].clone();

            // IP 증가
            self.frames[frame_idx].ip += 1;

            // 명령어 실행
            let result = self.execute_instruction(&inst, module, io)?;

            match result {
                super::instruction::ExecutionFlow::Continue => continue,
                super::instruction::ExecutionFlow::Return(Some(val)) => {
                    return Ok(val);
                }
                super::instruction::ExecutionFlow::Return(None) => {
                    return Ok(Value::None);
                }
                super::instruction::ExecutionFlow::WaitingForInput => {
                    // 동기적 실행 중에는 input()을 사용할 수 없음
                    return Err(err(
                        VmErrorKind::TypeError("input"),
                        "cannot use input() in builtin context".into(),
                    ));
                }
            }
        }
    }
}
