//! Dict methods implementation

use super::super::bytecode::Value;
use super::super::type_def::{BuiltinClassType, TYPE_DICT, TYPE_LIST};
use super::super::value::{BuiltinInstanceData, DictKey, Object, ObjectData};
use super::super::{VmError, VmErrorKind, VmResult, err};
use std::cell::RefCell;
use std::rc::Rc;

/// dict.get(key, default=None)
pub fn dict_get(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.is_empty() || args.len() > 2 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 2,
                got: args.len(),
            },
            format!("get() takes 1 or 2 arguments ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let key = value_to_dict_key(&args[0])?;
                let map_ref = map.borrow();

                match map_ref.get(&key) {
                    Some(value) => Ok(value.clone()),
                    None => {
                        if args.len() == 2 {
                            Ok(args[1].clone())
                        } else {
                            Ok(Value::None)
                        }
                    }
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.get"),
                    "get() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.get"),
            "get() requires a dict".to_string(),
        )),
    }
}

/// dict.keys()
pub fn dict_keys(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let map_ref = map.borrow();
                let keys: Vec<Value> = map_ref.keys().map(dict_key_to_value).collect();

                // 리스트로 반환
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_LIST,
                    ObjectData::List {
                        items: RefCell::new(keys),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.keys"),
                    "keys() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.keys"),
            "keys() requires a dict".to_string(),
        )),
    }
}

/// dict.values()
pub fn dict_values(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let map_ref = map.borrow();
                let values: Vec<Value> = map_ref.values().cloned().collect();

                // 리스트로 반환
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_LIST,
                    ObjectData::List {
                        items: RefCell::new(values),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.values"),
                    "values() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.values"),
            "values() requires a dict".to_string(),
        )),
    }
}

/// dict.clear()
pub fn dict_clear(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                map.borrow_mut().clear();
                Ok(Value::None)
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.clear"),
                    "clear() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.clear"),
            "clear() requires a dict".to_string(),
        )),
    }
}

/// dict.__iter__() - keys iterator
pub fn dict_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let map_ref = map.borrow();
                let keys: Vec<DictKey> = map_ref.keys().cloned().collect();

                // DictIterator 생성
                let iterator = Value::Object(Rc::new(Object::new(
                    TYPE_DICT,
                    ObjectData::BuiltinInstance {
                        class_type: BuiltinClassType::Dict,
                        data: BuiltinInstanceData::DictIterator {
                            keys,
                            current: RefCell::new(0),
                        },
                    },
                )));
                Ok(iterator)
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.__iter__"),
                    "__iter__() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.__iter__"),
            "__iter__() requires a dict".to_string(),
        )),
    }
}

/// dict iterator.__has_next__()
pub fn dict_has_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::Dict,
                data: BuiltinInstanceData::DictIterator { keys, current },
            } = &obj.data
            {
                let current_val = *current.borrow();
                Ok(Value::Bool(current_val < keys.len()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict iterator.__has_next__"),
                    "__has_next__() requires a dict iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict iterator.__has_next__"),
            "__has_next__() requires a dict iterator".to_string(),
        )),
    }
}

/// dict iterator.__next__()
pub fn dict_next(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::BuiltinInstance {
                class_type: BuiltinClassType::Dict,
                data: BuiltinInstanceData::DictIterator { keys, current },
            } = &obj.data
            {
                let mut current_mut = current.borrow_mut();

                if *current_mut >= keys.len() {
                    return Err(err(
                        VmErrorKind::TypeError("dict iterator.__next__"),
                        "StopIteration".to_string(),
                    ));
                }

                let key = &keys[*current_mut];
                *current_mut += 1;
                Ok(dict_key_to_value(key))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict iterator.__next__"),
                    "__next__() requires a dict iterator".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict iterator.__next__"),
            "__next__() requires a dict iterator".to_string(),
        )),
    }
}

/// dict.pop(key, default=None)
pub fn dict_pop(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.is_empty() || args.len() > 2 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 2,
                got: args.len(),
            },
            format!("pop() takes 1 or 2 arguments ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let key = value_to_dict_key(&args[0])?;
                let mut map_mut = map.borrow_mut();

                match map_mut.remove(&key) {
                    Some(value) => Ok(value),
                    None => {
                        if args.len() == 2 {
                            Ok(args[1].clone())
                        } else {
                            Ok(Value::None)
                        }
                    }
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.pop"),
                    "pop() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.pop"),
            "pop() requires a dict".to_string(),
        )),
    }
}

/// dict.update(other_dict)
pub fn dict_update(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("update() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                // other도 dict인지 확인
                match &args[0] {
                    Value::Object(other_obj) => {
                        if let ObjectData::Dict { map: other_map } = &other_obj.data {
                            let other_map_ref = other_map.borrow();
                            let mut map_mut = map.borrow_mut();

                            // other의 모든 key-value를 복사
                            for (key, value) in other_map_ref.iter() {
                                map_mut.insert(key.clone(), value.clone());
                            }

                            Ok(Value::None)
                        } else {
                            Err(err(
                                VmErrorKind::TypeError("dict.update"),
                                "update() argument must be a dict".to_string(),
                            ))
                        }
                    }
                    _ => Err(err(
                        VmErrorKind::TypeError("dict.update"),
                        "update() argument must be a dict".to_string(),
                    )),
                }
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.update"),
                    "update() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.update"),
            "update() requires a dict".to_string(),
        )),
    }
}

/// dict.items() - returns list of [key, value] pairs
pub fn dict_items(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    match receiver {
        Value::Object(obj) => {
            if let ObjectData::Dict { map } = &obj.data {
                let map_ref = map.borrow();
                let mut items = Vec::new();

                for (key, value) in map_ref.iter() {
                    // 각 (key, value) 쌍을 [key, value] 리스트로 만듦
                    let pair = vec![dict_key_to_value(key), value.clone()];
                    items.push(Value::Object(Rc::new(Object::new(
                        TYPE_LIST,
                        ObjectData::List {
                            items: RefCell::new(pair),
                        },
                    ))));
                }

                // 리스트의 리스트로 반환
                Ok(Value::Object(Rc::new(Object::new(
                    TYPE_LIST,
                    ObjectData::List {
                        items: RefCell::new(items),
                    },
                ))))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict.items"),
                    "items() requires a dict".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict.items"),
            "items() requires a dict".to_string(),
        )),
    }
}

// Helper functions
fn value_to_dict_key(value: &Value) -> VmResult<DictKey> {
    match value {
        Value::Int(i) => Ok(DictKey::Int(*i)),
        Value::Bool(b) => Ok(DictKey::Bool(*b)),
        Value::Object(obj) => {
            if let ObjectData::String(s) = &obj.data {
                Ok(DictKey::String(s.clone()))
            } else {
                Err(err(
                    VmErrorKind::TypeError("dict key"),
                    "Dict keys must be int, bool, or str".to_string(),
                ))
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("dict key"),
            "Dict keys must be int, bool, or str".to_string(),
        )),
    }
}

fn dict_key_to_value(key: &DictKey) -> Value {
    match key {
        DictKey::Int(i) => Value::Int(*i),
        DictKey::Bool(b) => Value::Bool(*b),
        DictKey::String(s) => {
            use super::super::type_def::make_string;
            make_string(s.clone())
        }
    }
}
