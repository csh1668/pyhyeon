//! TreeSet builtin constructor

use super::super::bytecode::Value;
use super::super::value::{Object, ObjectData, SetKey};
use super::super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::TYPE_TREESET;
use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

/// treeset() 생성자
///
/// treeset() - 빈 treeset 생성
/// treeset(iterable) - iterable의 원소들로 treeset 생성
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.is_empty() {
        // 빈 treeset 생성
        Ok(Value::Object(Rc::new(Object::new(
            TYPE_TREESET,
            ObjectData::TreeSet {
                items: RefCell::new(BTreeSet::new()),
            },
        ))))
    } else if args.len() == 1 {
        // iterable로부터 treeset 생성
        let iterable = &args[0];
        let mut treeset = BTreeSet::new();

        // iterable을 순회하며 원소 추가
        match iterable {
            Value::Object(obj) => {
                match &obj.data {
                    ObjectData::List { items } => {
                        for item in items.borrow().iter() {
                            let key = value_to_set_key(item)?;
                            treeset.insert(key);
                        }
                    }
                    ObjectData::Tuple { items } => {
                        for item in items.iter() {
                            let key = value_to_set_key(item)?;
                            treeset.insert(key);
                        }
                    }
                    ObjectData::Set { items } => {
                        // Set이면 복사
                        treeset = items.borrow().iter().cloned().collect();
                    }
                    ObjectData::TreeSet { items } => {
                        // 이미 TreeSet이면 복사
                        treeset = items.borrow().clone();
                    }
                    ObjectData::String(s) => {
                        // 문자열의 각 문자를 원소로 추가
                        for ch in s.chars() {
                            let str_obj = Value::Object(Rc::new(Object::new(
                                crate::builtins::TYPE_STR,
                                ObjectData::String(ch.to_string()),
                            )));
                            let key = value_to_set_key(&str_obj)?;
                            treeset.insert(key);
                        }
                    }
                    _ => {
                        return Err(err(
                            VmErrorKind::TypeError("treeset"),
                            format!("treeset() argument must be iterable"),
                        ));
                    }
                }
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("treeset"),
                    format!("treeset() argument must be iterable"),
                ));
            }
        }

        Ok(Value::Object(Rc::new(Object::new(
            TYPE_TREESET,
            ObjectData::TreeSet {
                items: RefCell::new(treeset),
            },
        ))))
    } else {
        Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("treeset() takes 0 or 1 argument ({} given)", args.len()),
        ))
    }
}

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

