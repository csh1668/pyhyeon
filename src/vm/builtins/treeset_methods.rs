//! TreeSet methods implementation

use super::super::bytecode::Value;
use super::super::value::{BuiltinInstanceData, Object, ObjectData, SetKey};
use super::super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::{TYPE_TREESET, TYPE_LIST};
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::ops::Bound;
use std::rc::Rc;

/// Helper: Value를 SetKey로 변환
fn value_to_set_key(value: &Value) -> VmResult<SetKey> {
    match value {
        Value::Int(i) => Ok(SetKey::Int(*i)),
        Value::Bool(b) => Ok(SetKey::Bool(*b)),
        Value::Object(obj) => {
            if let ObjectData::String(s) = &obj.data {
                Ok(SetKey::String(s.clone()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset element"),
                    "TreeSet elements must be int, bool, or str".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset element"),
            "TreeSet elements must be int, bool, or str".to_string(),
        )),
    }
}

/// Helper: SetKey를 Value로 변환
fn set_key_to_value(key: &SetKey) -> Value {
    match key {
        SetKey::Int(i) => Value::Int(*i),
        SetKey::Bool(b) => Value::Bool(*b),
        SetKey::String(s) => Value::Object(Rc::new(Object::new(
            crate::builtins::TYPE_STR,
            ObjectData::String(s.clone()),
        ))),
    }
}

/// treeset.add(item)
pub fn treeset_add(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("add() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                items.borrow_mut().insert(key);
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.add"),
                    "add() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.add"),
            "add() requires a treeset".to_string(),
        )),
    }
}

/// treeset.remove(item)
pub fn treeset_remove(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("remove() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                if items.borrow_mut().remove(&key) {
                    Ok(Value::None)
                } else {
                    Err(err(
                        VmErrorKind::TypeError("treeset.remove"),
                        format!("Element not found in treeset"),
                    ))
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.remove"),
                    "remove() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.remove"),
            "remove() requires a treeset".to_string(),
        )),
    }
}

/// treeset.contains(item)
pub fn treeset_contains(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("contains() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                Ok(Value::Bool(items.borrow().contains(&key)))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.contains"),
                    "contains() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.contains"),
            "contains() requires a treeset".to_string(),
        )),
    }
}

/// treeset.union(other)
pub fn treeset_union(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("union() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let other_treeset = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::TreeSet { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("treeset.union"),
                                "union() requires a treeset argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("treeset.union"),
                            "union() requires a treeset argument".to_string(),
                        ));
                    }
                };

                let mut result = items.borrow().clone();
                result.extend(other_treeset);

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_TREESET,
                    ObjectData::TreeSet {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.union"),
                    "union() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.union"),
            "union() requires a treeset".to_string(),
        )),
    }
}

/// treeset.intersection(other)
pub fn treeset_intersection(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("intersection() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let other_treeset = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::TreeSet { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("treeset.intersection"),
                                "intersection() requires a treeset argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("treeset.intersection"),
                            "intersection() requires a treeset argument".to_string(),
                        ));
                    }
                };

                let result: BTreeSet<SetKey> = items
                    .borrow()
                    .intersection(&other_treeset)
                    .cloned()
                    .collect();

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_TREESET,
                    ObjectData::TreeSet {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.intersection"),
                    "intersection() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.intersection"),
            "intersection() requires a treeset".to_string(),
        )),
    }
}

/// treeset.difference(other)
pub fn treeset_difference(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("difference() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let other_treeset = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::TreeSet { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("treeset.difference"),
                                "difference() requires a treeset argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("treeset.difference"),
                            "difference() requires a treeset argument".to_string(),
                        ));
                    }
                };

                let result: BTreeSet<SetKey> = items
                    .borrow()
                    .difference(&other_treeset)
                    .cloned()
                    .collect();

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_TREESET,
                    ObjectData::TreeSet {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.difference"),
                    "difference() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.difference"),
            "difference() requires a treeset".to_string(),
        )),
    }
}

/// treeset.clear()
pub fn treeset_clear(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                items.borrow_mut().clear();
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.clear"),
                    "clear() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.clear"),
            "clear() requires a treeset".to_string(),
        )),
    }
}

/// treeset.copy()
pub fn treeset_copy(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let copied = items.borrow().clone();
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_TREESET,
                    ObjectData::TreeSet {
                        items: RefCell::new(copied),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.copy"),
                    "copy() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.copy"),
            "copy() requires a treeset".to_string(),
        )),
    }
}

/// treeset.lower_bound(value) - value 이상인 최소 원소 반환
pub fn treeset_lower_bound(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("lower_bound() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                let items_ref = items.borrow();
                
                // BTreeSet의 range를 사용하여 key 이상인 첫 번째 원소 찾기
                if let Some(found) = items_ref.range(key..).next() {
                    Ok(set_key_to_value(found))
                } else {
                    Ok(Value::None)
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.lower_bound"),
                    "lower_bound() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.lower_bound"),
            "lower_bound() requires a treeset".to_string(),
        )),
    }
}

/// treeset.upper_bound(value) - value 초과인 최소 원소 반환
pub fn treeset_upper_bound(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("upper_bound() takes 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                let items_ref = items.borrow();

                if let Some(found) = items_ref.range((Bound::Excluded(key), Bound::Unbounded)).next() {
                  return Ok(set_key_to_value(found));
                }
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.upper_bound"),
                    "upper_bound() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.upper_bound"),
            "upper_bound() requires a treeset".to_string(),
        )),
    }
}

/// treeset.min() - 최소 원소 반환
pub fn treeset_min(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let items_ref = items.borrow();
                if let Some(min) = items_ref.first() {
                    Ok(set_key_to_value(min))
                } else {
                    Ok(Value::None)
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.min"),
                    "min() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.min"),
            "min() requires a treeset".to_string(),
        )),
    }
}

/// treeset.max() - 최대 원소 반환
pub fn treeset_max(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let items_ref = items.borrow();
                if let Some(max) = items_ref.last() {
                    Ok(set_key_to_value(max))
                } else {
                    Ok(Value::None)
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.max"),
                    "max() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.max"),
            "max() requires a treeset".to_string(),
        )),
    }
}

/// treeset.__iter__()
pub fn treeset_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::TreeSet { items } = &obj.data {
                let keys: Vec<SetKey> = items.borrow().iter().cloned().collect();
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_TREESET,
                    ObjectData::BuiltinInstance {
                        class_type: crate::builtins::BuiltinClassType::TreeSet,
                        data: BuiltinInstanceData::TreeSetIterator {
                            keys,
                            current: RefCell::new(0),
                        },
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.__iter__"),
                    "__iter__() requires a treeset".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.__iter__"),
            "__iter__() requires a treeset".to_string(),
        )),
    }
}

/// treeset.__has_next__()
pub fn treeset_has_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: crate::builtins::BuiltinClassType::TreeSet,
                data: BuiltinInstanceData::TreeSetIterator { keys, current },
            } = &obj.data
            {
                Ok(Value::Bool(*current.borrow() < keys.len()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.__has_next__"),
                    "__has_next__() requires a treeset iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.__has_next__"),
            "__has_next__() requires a treeset iterator".to_string(),
        )),
    }
}

/// treeset.__next__()
pub fn treeset_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: crate::builtins::BuiltinClassType::TreeSet,
                data: BuiltinInstanceData::TreeSetIterator { keys, current },
            } = &obj.data
            {
                let idx = *current.borrow();
                if idx >= keys.len() {
                    return Err(err(
                        VmErrorKind::TypeError("treeset.__next__"),
                        "StopIteration".to_string(),
                    ));
                }
                *current.borrow_mut() += 1;
                Ok(set_key_to_value(&keys[idx]))
            } else {
                Err(err(
                    VmErrorKind::TypeError("treeset.__next__"),
                    "__next__() requires a treeset iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("treeset.__next__"),
            "__next__() requires a treeset iterator".to_string(),
        )),
    }
}

