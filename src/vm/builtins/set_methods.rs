//! Set methods implementation

use super::super::bytecode::Value;
use super::super::value::{BuiltinInstanceData, Object, ObjectData, SetKey};
use super::super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::{TYPE_SET, TYPE_LIST};
use std::cell::RefCell;
use std::collections::HashSet;
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
                    VmErrorKind::TypeError("set element"),
                    "Set elements must be int, bool, or str".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set element"),
            "Set elements must be int, bool, or str".to_string(),
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

/// set.add(item)
pub fn set_add(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                items.borrow_mut().insert(key);
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.add"),
                    "add() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.add"),
            "add() requires a set".to_string(),
        )),
    }
}

/// set.remove(item)
pub fn set_remove(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                if items.borrow_mut().remove(&key) {
                    Ok(Value::None)
                } else {
                    Err(err(
                        VmErrorKind::TypeError("set.remove"),
                        format!("Element not found in set"),
                    ))
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.remove"),
                    "remove() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.remove"),
            "remove() requires a set".to_string(),
        )),
    }
}

/// set.contains(item)
pub fn set_contains(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let key = value_to_set_key(&args[0])?;
                Ok(Value::Bool(items.borrow().contains(&key)))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.contains"),
                    "contains() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.contains"),
            "contains() requires a set".to_string(),
        )),
    }
}

/// set.union(other)
pub fn set_union(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let other_set = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::Set { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("set.union"),
                                "union() requires a set argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("set.union"),
                            "union() requires a set argument".to_string(),
                        ));
                    }
                };

                let mut result = items.borrow().clone();
                result.extend(other_set);

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_SET,
                    ObjectData::Set {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.union"),
                    "union() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.union"),
            "union() requires a set".to_string(),
        )),
    }
}

/// set.intersection(other)
pub fn set_intersection(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let other_set = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::Set { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("set.intersection"),
                                "intersection() requires a set argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("set.intersection"),
                            "intersection() requires a set argument".to_string(),
                        ));
                    }
                };

                let result: HashSet<SetKey> = items
                    .borrow()
                    .intersection(&other_set)
                    .cloned()
                    .collect();

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_SET,
                    ObjectData::Set {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.intersection"),
                    "intersection() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.intersection"),
            "intersection() requires a set".to_string(),
        )),
    }
}

/// set.difference(other)
pub fn set_difference(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
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
            if let ObjectData::Set { items } = &obj.data {
                let other_set = match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::Set { items: other_items } = &other_obj.data {
                            other_items.borrow().clone()
                        } else {
                            return Err(err(
                                VmErrorKind::TypeError("set.difference"),
                                "difference() requires a set argument".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("set.difference"),
                            "difference() requires a set argument".to_string(),
                        ));
                    }
                };

                let result: HashSet<SetKey> = items
                    .borrow()
                    .difference(&other_set)
                    .cloned()
                    .collect();

                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_SET,
                    ObjectData::Set {
                        items: RefCell::new(result),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.difference"),
                    "difference() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.difference"),
            "difference() requires a set".to_string(),
        )),
    }
}

/// set.clear()
pub fn set_clear(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Set { items } = &obj.data {
                items.borrow_mut().clear();
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.clear"),
                    "clear() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.clear"),
            "clear() requires a set".to_string(),
        )),
    }
}

/// set.copy()
pub fn set_copy(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Set { items } = &obj.data {
                let copied = items.borrow().clone();
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_SET,
                    ObjectData::Set {
                        items: RefCell::new(copied),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.copy"),
                    "copy() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.copy"),
            "copy() requires a set".to_string(),
        )),
    }
}

/// set.__iter__()
pub fn set_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Set { items } = &obj.data {
                let keys: Vec<SetKey> = items.borrow().iter().cloned().collect();
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_SET,
                    ObjectData::BuiltinInstance {
                        class_type: crate::builtins::BuiltinClassType::Set,
                        data: BuiltinInstanceData::SetIterator {
                            keys,
                            current: RefCell::new(0),
                        },
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.__iter__"),
                    "__iter__() requires a set".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.__iter__"),
            "__iter__() requires a set".to_string(),
        )),
    }
}

/// set.__has_next__()
pub fn set_has_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: crate::builtins::BuiltinClassType::Set,
                data: BuiltinInstanceData::SetIterator { keys, current },
            } = &obj.data
            {
                Ok(Value::Bool(*current.borrow() < keys.len()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.__has_next__"),
                    "__has_next__() requires a set iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.__has_next__"),
            "__has_next__() requires a set iterator".to_string(),
        )),
    }
}

/// set.__next__()
pub fn set_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: crate::builtins::BuiltinClassType::Set,
                data: BuiltinInstanceData::SetIterator { keys, current },
            } = &obj.data
            {
                let idx = *current.borrow();
                if idx >= keys.len() {
                    return Err(err(
                        VmErrorKind::TypeError("set.__next__"),
                        "StopIteration".to_string(),
                    ));
                }
                *current.borrow_mut() += 1;
                Ok(set_key_to_value(&keys[idx]))
            } else {
                Err(err(
                    VmErrorKind::TypeError("set.__next__"),
                    "__next__() requires a set iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("set.__next__"),
            "__next__() requires a set iterator".to_string(),
        )),
    }
}

