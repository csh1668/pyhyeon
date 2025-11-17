//! map() builtin function and map iterator implementation

use super::super::bytecode::{Module, Value};
use super::super::value::{BuiltinInstanceData, Object, ObjectData};
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::type_name;
use crate::builtins::{BuiltinClassType, TYPE_MAP_ITER};
use crate::runtime_io::RuntimeIo;
use std::rc::Rc;

/// map(func, iterable) 생성자
///
/// 각 요소에 함수를 적용하는 iterator를 반환합니다.
///
/// ```python
/// numbers = [1, 2, 3, 4, 5]
/// squared = map(lambda x: x * x, numbers)
/// for n in squared:
///     print(n)  # 1, 4, 9, 16, 25
/// ```
pub fn create_map(args: Vec<Value>) -> VmResult<Value> {
    // Arity는 이미 builtin registry에서 검증됨 (2개)
    let func = args[0].clone();
    let iterable = args[1].clone();

    // func가 callable인지 검증
    if !is_callable(&func) {
        return Err(err(
            VmErrorKind::TypeError("map"),
            format!(
                "map() argument 1 must be callable, not '{}'",
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
                    VmErrorKind::TypeError("map"),
                    format!(
                        "map() argument 2 must be iterable, not '{}'",
                        type_name(&iterable)
                    ),
                ));
            }
        },
        _ => {
            return Err(err(
                VmErrorKind::TypeError("map"),
                format!(
                    "map() argument 2 must be iterable, not '{}'",
                    type_name(&iterable)
                ),
            ));
        }
    };

    // MapIterator 생성
    Ok(Value::Object(Rc::new(Object::new(
        TYPE_MAP_ITER,
        ObjectData::BuiltinInstance {
            class_type: BuiltinClassType::MapIter,
            data: BuiltinInstanceData::MapIterator {
                func: Box::new(func),
                source_iter: Box::new(source_iter),
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

/// map_iterator.__iter__()
///
/// Iterator protocol: 자기 자신을 반환
pub fn map_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    // Iterator는 자기 자신을 반환
    Ok(receiver.clone())
}

/// map_iterator.__has_next__()
///
/// Source iterator에 다음 요소가 있는지 확인
pub fn map_has_next<IO: RuntimeIo>(
    receiver: &Value,
    _args: Vec<Value>,
    module: &mut Module,
    vm: &mut super::super::machine::Vm,
    io: &mut IO,
) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::MapIter,
                data: BuiltinInstanceData::MapIterator { source_iter, .. },
            } = &obj.data
            {
                // source_iter.__has_next__() 호출
                vm.call_method(source_iter.as_ref(), "__has_next__", vec![], module, io)
            } else {
                Err(err(
                    VmErrorKind::TypeError("map_iterator"),
                    "expected map_iterator".into(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("map_iterator"),
            "expected map_iterator".into(),
        )),
    }
}

/// map_iterator.__next__()
///
/// Source iterator의 다음 요소에 함수를 적용하여 반환
pub fn map_next<IO: RuntimeIo>(
    receiver: &Value,
    _args: Vec<Value>,
    module: &mut Module,
    vm: &mut super::super::machine::Vm,
    io: &mut IO,
) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::MapIter,
                data:
                    BuiltinInstanceData::MapIterator {
                        func, source_iter, ..
                    },
            } = &obj.data
            {
                // 1. source_iter.__next__() 호출하여 값 획득
                let value = vm.call_method(source_iter, "__next__", vec![], module, io)?;

                // 2. func(value) 호출
                let result = vm.call_function(func, vec![value], module, io)?;

                Ok(result)
            } else {
                Err(err(
                    VmErrorKind::TypeError("map_iterator"),
                    "expected map_iterator".into(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("map_iterator"),
            "expected map_iterator".into(),
        )),
    }
}
