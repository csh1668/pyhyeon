//! List methods implementation

use super::super::bytecode::Value;
use super::super::type_def::{BuiltinClassType, TYPE_LIST};
use super::super::value::{BuiltinInstanceData, Object, ObjectData};
use super::super::{VmError, VmErrorKind, VmResult, err};
use std::cell::RefCell;
use std::rc::Rc;

/// list.append(item)
pub fn list_append(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("append() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                items.borrow_mut().push(args[0].clone());
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.append"),
                    "append() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.append"),
            "append() requires a list".to_string(),
        )),
    }
}

/// list.pop([index])
pub fn list_pop(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() > 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("pop() takes at most 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let mut items_mut = items.borrow_mut();

                if items_mut.is_empty() {
                    return Err(err(
                        VmErrorKind::TypeError("list.pop"),
                        "pop from empty list".to_string(),
                    ));
                }

                let index = if args.is_empty() {
                    items_mut.len() - 1
                } else {
                    match args[0] {
                        Value::Int(i) => {
                            let len = items_mut.len() as i64;
                            let actual_idx = if i < 0 {
                                (len + i) as usize
                            } else {
                                i as usize
                            };
                            if actual_idx >= items_mut.len() {
                                return Err(err(
                                    VmErrorKind::TypeError("list.pop"),
                                    format!("pop index out of range: {}", i),
                                ));
                            }
                            actual_idx
                        }
                        _ => {
                            return Err(err(
                                VmErrorKind::TypeError("list.pop"),
                                "pop() index must be an integer".to_string(),
                            ));
                        }
                    }
                };

                Ok(items_mut.remove(index))
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.pop"),
                    "pop() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.pop"),
            "pop() requires a list".to_string(),
        )),
    }
}

/// list.extend(iterable)
pub fn list_extend(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("extend() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                // iterable이 리스트인지 확인
                match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::List {
                            items: ref other_items,
                        } = other_obj.data
                        {
                            let other_vec = other_items.borrow().clone();
                            items.borrow_mut().extend(other_vec);
                            Ok(Value::None)
                        } else {
                            Err(err(
                                VmErrorKind::TypeError("list.extend"),
                                "extend() argument must be iterable".to_string(),
                            ))
                        }
                    }
                    _ => Err(err(
                        VmErrorKind::TypeError("list.extend"),
                        "extend() argument must be iterable".to_string(),
                    )),
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.extend"),
                    "extend() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.extend"),
            "extend() requires a list".to_string(),
        )),
    }
}

/// list.insert(index, item)
pub fn list_insert(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 2 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 2,
                got: args.len(),
            },
            format!("insert() takes exactly 2 arguments ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let index = match args[0] {
                    Value::Int(i) => {
                        let len = items.borrow().len() as i64;

                        if i < 0 {
                            0.max((len + i) as usize)
                        } else {
                            let items_len = items.borrow().len();
                            (i as usize).min(items_len)
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("list.insert"),
                            "insert() index must be an integer".to_string(),
                        ));
                    }
                };

                items.borrow_mut().insert(index, args[1].clone());
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.insert"),
                    "insert() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.insert"),
            "insert() requires a list".to_string(),
        )),
    }
}

/// list.remove(item)
pub fn list_remove(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("remove() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let mut items_mut = items.borrow_mut();

                // 첫 번째로 일치하는 항목 찾기
                for (i, item) in items_mut.iter().enumerate() {
                    if super::super::utils::eq_vals(item, &args[0]) {
                        items_mut.remove(i);
                        return Ok(Value::None);
                    }
                }

                // 찾지 못한 경우
                Err(err(
                    VmErrorKind::TypeError("list.remove"),
                    "list.remove(x): x not in list".to_string(),
                ))
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.remove"),
                    "remove() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.remove"),
            "remove() requires a list".to_string(),
        )),
    }
}

/// list.reverse()
pub fn list_reverse(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                items.borrow_mut().reverse();
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.reverse"),
                    "reverse() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.reverse"),
            "reverse() requires a list".to_string(),
        )),
    }
}

/// list.sort()
pub fn list_sort(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let mut items_mut = items.borrow_mut();

                // 정수 리스트만 정렬 지원
                let all_ints = items_mut.iter().all(|v| matches!(v, Value::Int(_)));

                if !all_ints {
                    return Err(err(
                        VmErrorKind::TypeError("list.sort"),
                        "sort() currently only supports lists of integers".to_string(),
                    ));
                }

                items_mut.sort_by_key(|v| if let Value::Int(i) = v { *i } else { 0 });

                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.sort"),
                    "sort() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.sort"),
            "sort() requires a list".to_string(),
        )),
    }
}

/// list.clear()
pub fn list_clear(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                items.borrow_mut().clear();
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.clear"),
                    "clear() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.clear"),
            "clear() requires a list".to_string(),
        )),
    }
}

/// list.index(item)
pub fn list_index(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("index() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let items_ref = items.borrow();

                for (i, item) in items_ref.iter().enumerate() {
                    if super::super::utils::eq_vals(item, &args[0]) {
                        return Ok(Value::Int(i as i64));
                    }
                }

                Err(err(
                    VmErrorKind::TypeError("list.index"),
                    "list.index(x): x not in list".to_string(),
                ))
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.index"),
                    "index() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.index"),
            "index() requires a list".to_string(),
        )),
    }
}

/// list.count(item)
pub fn list_count(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("count() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                let items_ref = items.borrow();
                let count = items_ref
                    .iter()
                    .filter(|item| super::super::utils::eq_vals(item, &args[0]))
                    .count();

                Ok(Value::Int(count as i64))
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.count"),
                    "count() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.count"),
            "count() requires a list".to_string(),
        )),
    }
}

/// list.__iter__()
pub fn list_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::List { items } = &obj.data {
                // ListIterator 생성
                let iterator = Value::Object(Rc::new(Object::new(
                    TYPE_LIST,
                    ObjectData::BuiltinInstance {
                        class_type: BuiltinClassType::List,
                        data: BuiltinInstanceData::ListIterator {
                            items: Rc::new(RefCell::clone(items)),
                            current: RefCell::new(0),
                        },
                    },
                )));
                Ok(iterator)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list.__iter__"),
                    "__iter__() requires a list".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list.__iter__"),
            "__iter__() requires a list".to_string(),
        )),
    }
}

/// list iterator.__has_next__()
pub fn list_has_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::List,
                data: BuiltinInstanceData::ListIterator { items, current },
            } = &obj.data
            {
                let items_ref = items.borrow();
                let current_val = *current.borrow();
                Ok(Value::Bool(current_val < items_ref.len()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("list iterator.__has_next__"),
                    "__has_next__() requires a list iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list iterator.__has_next__"),
            "__has_next__() requires a list iterator".to_string(),
        )),
    }
}

/// list iterator.__next__()
pub fn list_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::List,
                data: BuiltinInstanceData::ListIterator { items, current },
            } = &obj.data
            {
                let items_ref = items.borrow();
                let mut current_mut = current.borrow_mut();

                if *current_mut >= items_ref.len() {
                    return Err(err(
                        VmErrorKind::TypeError("list iterator.__next__"),
                        "StopIteration".to_string(),
                    ));
                }

                let value = items_ref[*current_mut].clone();
                *current_mut += 1;
                Ok(value)
            } else {
                Err(err(
                    VmErrorKind::TypeError("list iterator.__next__"),
                    "__next__() requires a list iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("list iterator.__next__"),
            "__next__() requires a list iterator".to_string(),
        )),
    }
}
