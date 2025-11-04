use super::super::bytecode::{Module, Value};
use super::super::type_def::{Arity, MethodImpl, TYPE_BOOL, TYPE_INT, TYPE_NONE, TYPE_STR};
use super::super::value::ObjectData;
use super::{Vm, VmErrorKind, VmResult, err};
use crate::runtime_io::RuntimeIo;

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
        module: &Module,
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
        module: &Module,
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
    ///
    /// # 구현된 메서드
    ///
    /// - **String**: upper, lower, strip, replace, startswith, endswith, find, count
    /// - **Range**: __iter__
    ///
    /// # 미구현 메서드
    ///
    /// - split, join (리스트 타입 필요)
    pub(super) fn call_native_method_dispatch(
        &self,
        method: super::super::type_def::NativeMethod,
        receiver: &Value,
        args: Vec<Value>,
        _module: &Module,
        _io: &mut dyn RuntimeIo,
    ) -> VmResult<Value> {
        use super::super::builtins::{dict_methods, int, list_methods, range, str_builtin};
        use super::super::type_def::NativeMethod as NM;

        // builtins 모듈에서 직접 호출
        match method {
            // Int 매직 메서드들
            // 실제로는 사용되지 않으나, Dynamic Dispatch를 위해 남겨둠
            NM::IntAdd => int::int_add(receiver, args),
            NM::IntSub => int::int_sub(receiver, args),
            NM::IntMul => int::int_mul(receiver, args),
            NM::IntFloorDiv => int::int_floordiv(receiver, args),
            NM::IntMod => int::int_mod(receiver, args),
            NM::IntNeg => int::int_neg(receiver, args),
            NM::IntPos => int::int_pos(receiver, args),
            NM::IntLt => int::int_lt(receiver, args),
            NM::IntLe => int::int_le(receiver, args),
            NM::IntGt => int::int_gt(receiver, args),
            NM::IntGe => int::int_ge(receiver, args),
            NM::IntEq => int::int_eq(receiver, args),
            NM::IntNe => int::int_ne(receiver, args),

            // String 매직 메서드들
            NM::StrAdd => str_builtin::str_add(receiver, args),
            NM::StrMul => str_builtin::str_mul(receiver, args),
            NM::StrLt => str_builtin::str_lt(receiver, args),
            NM::StrLe => str_builtin::str_le(receiver, args),
            NM::StrGt => str_builtin::str_gt(receiver, args),
            NM::StrGe => str_builtin::str_ge(receiver, args),
            NM::StrEq => str_builtin::str_eq(receiver, args),
            NM::StrNe => str_builtin::str_ne(receiver, args),

            // String 일반 메서드들
            NM::StrUpper => str_builtin::str_upper(receiver, args),
            NM::StrLower => str_builtin::str_lower(receiver, args),
            NM::StrStrip => str_builtin::str_strip(receiver, args),
            NM::StrSplit => str_builtin::str_split(receiver, args),
            NM::StrJoin => str_builtin::str_join(receiver, args),
            NM::StrReplace => str_builtin::str_replace(receiver, args),
            NM::StrStartsWith => str_builtin::str_starts_with(receiver, args),
            NM::StrEndsWith => str_builtin::str_ends_with(receiver, args),
            NM::StrFind => str_builtin::str_find(receiver, args),
            NM::StrCount => str_builtin::str_count(receiver, args),

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
        }
    }

    /// Value에서 String 데이터 추출
    ///
    /// Object의 ObjectData::String에서 문자열 참조를 가져옵니다.
    pub(super) fn expect_string<'a>(&self, v: &'a Value) -> VmResult<&'a str> {
        match v {
            Value::Object(obj) => match &obj.data {
                ObjectData::String(s) => Ok(s),
                _ => Err(err(
                    VmErrorKind::TypeError("str"),
                    "expected string object".into(),
                )),
            },
            _ => Err(err(VmErrorKind::TypeError("str"), "expected String".into())),
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
}
