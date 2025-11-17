//! filter() builtin function and filter iterator implementation

use super::super::bytecode::{Module, Value};
use super::super::value::{BuiltinInstanceData, Object, ObjectData};
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::type_name;
use crate::builtins::{BuiltinClassType, TYPE_FILTER_ITER};
use crate::runtime_io::RuntimeIo;
use crate::vm::builtins::bool::to_bool;
use std::rc::Rc;

/// filter(func, iterable) 생성자
///
/// 조건을 만족하는 요소만 선택하는 iterator를 반환합니다.
///
/// ```python
/// numbers = range(10)
/// evens = filter(lambda x: x % 2 == 0, numbers)
/// for n in evens:
///     print(n)  # 0, 2, 4, 6, 8
/// ```
pub fn create_filter(args: Vec<Value>) -> VmResult<Value> {
    // Arity는 이미 builtin registry에서 검증됨 (2개)
    let func = args[0].clone();
    let iterable = args[1].clone();

    // func가 callable인지 검증
    if !is_callable(&func) {
        return Err(err(
            VmErrorKind::TypeError("filter"),
            format!(
                "filter() argument 1 must be callable, not '{}'",
                type_name(&func)
            ),
        ));
    }

    // iterable을 iterator로 변환
    // List나 Dict의 경우 __iter__()를 직접 호출하여 iterator 생성
    let source_iter = match &iterable {
        Value::Object(obj) => match &obj.data {
            ObjectData::List { items } => {
                // ListIterator 생성
                use super::super::value::BuiltinInstanceData;
                use crate::builtins::TYPE_LIST;
                use std::cell::RefCell;

                Value::Object(Rc::new(super::super::value::Object::new(
                    TYPE_LIST,
                    ObjectData::BuiltinInstance {
                        class_type: BuiltinClassType::List,
                        data: BuiltinInstanceData::ListIterator {
                            items: Rc::new(RefCell::clone(items)),
                            current: RefCell::new(0),
                        },
                    },
                )))
            }
            // Range는 이미 iterable
            ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::Range,
                ..
            } => iterable.clone(),
            // 다른 iterator들도 그대로 사용
            ObjectData::BuiltinInstance { .. } => iterable.clone(),
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("filter"),
                    format!(
                        "filter() argument 2 must be iterable, not '{}'",
                        type_name(&iterable)
                    ),
                ));
            }
        },
        _ => {
            return Err(err(
                VmErrorKind::TypeError("filter"),
                format!(
                    "filter() argument 2 must be iterable, not '{}'",
                    type_name(&iterable)
                ),
            ));
        }
    };

    // FilterIterator 생성
    Ok(Value::Object(Rc::new(Object::new(
        TYPE_FILTER_ITER,
        ObjectData::BuiltinInstance {
            class_type: BuiltinClassType::FilterIter,
            data: BuiltinInstanceData::FilterIterator {
                func: Box::new(func),
                source_iter: Box::new(source_iter),
                peeked: std::cell::RefCell::new(None),
            },
        },
    ))))
}

/// 값이 callable인지 확인
fn is_callable(value: &Value) -> bool {
    match value {
        Value::Object(obj) => matches!(
            obj.data,
            ObjectData::UserFunction { .. } | ObjectData::BuiltinClass { .. }
        ),
        _ => false,
    }
}

// ========== Iterator Protocol 메서드들 ==========

/// filter_iterator.__iter__()
///
/// Iterator protocol: 자기 자신을 반환
pub fn filter_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    // Iterator는 자기 자신을 반환
    Ok(receiver.clone())
}

/// filter_iterator.__has_next__()
///
///
/// Source iterator에서 조건을 만족하는 다음 요소가 있는지 확인
///
/// NOTE: filter는 미리보기(peek)가 필요하므로 구현이 복잡함.
/// 여기서는 단순히 source iterator의 has_next를 반환하고,
/// 실제 필터링은 __next__()에서 수행
pub fn filter_has_next<IO: RuntimeIo>(
    receiver: &Value,
    _args: Vec<Value>,
    module: &mut Module,
    vm: &mut super::super::machine::Vm,
    io: &mut IO,
) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::FilterIter,
                data: BuiltinInstanceData::FilterIterator { func, source_iter, peeked },
            } = &obj.data
            {
                // 이미 peek한 값이 있으면 True 반환
                if peeked.borrow().is_some() {
                    return Ok(Value::Bool(true));
                }

                // 조건을 만족하는 다음 요소를 찾아서 peeked에 저장
                loop {
                    let has_next = vm.call_method(source_iter.as_ref(), "__has_next__", vec![], module, io)?;

                    if !matches!(has_next, Value::Bool(true)) {
                        // 더 이상 요소가 없음
                        return Ok(Value::Bool(false));
                    }

                    let value = vm.call_method(source_iter.as_ref(), "__next__", vec![], module, io)?;
                    let predicate_result = vm.call_function(func.as_ref(), vec![value.clone()], module, io)?;

                    if to_bool(&predicate_result) {
                        // 조건을 만족하는 요소를 찾음 - peeked에 저장
                        *peeked.borrow_mut() = Some(value);
                        return Ok(Value::Bool(true));
                    }

                    // 조건을 만족하지 않으면 다음 요소 검사
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("filter_iterator"),
                    "expected filter_iterator".into(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("filter_iterator"),
            "expected filter_iterator".into(),
        )),
    }
}

/// filter_iterator.__next__()
///
/// Source iterator에서 조건을 만족하는 다음 요소를 반환
pub fn filter_next<IO: RuntimeIo>(
    receiver: &Value,
    _args: Vec<Value>,
    module: &mut Module,
    vm: &mut super::super::machine::Vm,
    io: &mut IO,
) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::FilterIter,
                data:
                    BuiltinInstanceData::FilterIterator {
                        func, source_iter, peeked
                    },
            } = &obj.data
            {
                // peeked에 값이 있으면 그것을 반환 (has_next에서 미리 찾아둠)
                if let Some(value) = peeked.borrow_mut().take() {
                    return Ok(value);
                }

                // peeked가 없으면 직접 찾기 (has_next가 호출되지 않은 경우)
                loop {
                    let has_next =
                        vm.call_method(source_iter.as_ref(), "__has_next__", vec![], module, io)?;

                    if !matches!(has_next, Value::Bool(true)) {
                        // Iterator 소진 - StopIteration 에러
                        return Err(err(
                            VmErrorKind::TypeError("filter_iterator"),
                            "StopIteration".into(),
                        ));
                    }

                    let value = vm.call_method(source_iter.as_ref(), "__next__", vec![], module, io)?;
                    let predicate_result = vm.call_function(func.as_ref(), vec![value.clone()], module, io)?;

                    if to_bool(&predicate_result) {
                        return Ok(value);
                    }
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("filter_iterator"),
                    "expected filter_iterator".into(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("filter_iterator"),
            "expected filter_iterator".into(),
        )),
    }
}