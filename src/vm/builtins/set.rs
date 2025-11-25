//! Set builtin constructor

use super::super::bytecode::Value;
use super::super::value::{Object, ObjectData, SetKey};
use super::super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::TYPE_SET;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

/// set() 생성자
///
/// set() - 빈 set 생성
/// set(iterable) - iterable의 원소들로 set 생성
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.is_empty() {
        // 빈 set 생성
        Ok(Value::Object(Rc::new(Object::new(
            TYPE_SET,
            ObjectData::Set {
                items: RefCell::new(HashSet::new()),
            },
        ))))
    } else if args.len() == 1 {
        // iterable로부터 set 생성
        let iterable = &args[0];
        let mut set = HashSet::new();

        // iterable을 순회하며 원소 추가
        // Value가 iterable인지 확인하고 순회
        match iterable {
            Value::Object(obj) => {
                // __iter__ 메서드 호출을 위해 VM이 필요하지만,
                // 여기서는 직접 처리할 수 있는 타입들을 먼저 처리
                match &obj.data {
                    ObjectData::List { items } => {
                        for item in items.borrow().iter() {
                            let key = value_to_set_key(item)?;
                            set.insert(key);
                        }
                    }
                    ObjectData::Tuple { items } => {
                        for item in items.iter() {
                            let key = value_to_set_key(item)?;
                            set.insert(key);
                        }
                    }
                    ObjectData::Set { items } => {
                        // 이미 set이면 복사
                        set = items.borrow().clone();
                    }
                    ObjectData::TreeSet { items } => {
                        // TreeSet이면 복사
                        set = items.borrow().iter().cloned().collect();
                    }
                    ObjectData::String(s) => {
                        // 문자열의 각 문자를 원소로 추가
                        for ch in s.chars() {
                            let str_obj = Value::Object(Rc::new(Object::new(
                                crate::builtins::TYPE_STR,
                                ObjectData::String(ch.to_string()),
                            )));
                            let key = value_to_set_key(&str_obj)?;
                            set.insert(key);
                        }
                    }
                    _ => {
                        // 다른 iterable 타입은 나중에 iterator를 통해 처리
                        // 현재는 에러 반환
                        return Err(err(
                            VmErrorKind::TypeError("set"),
                            format!("set() argument must be iterable"),
                        ));
                    }
                }
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("set"),
                    format!("set() argument must be iterable"),
                ));
            }
        }

        Ok(Value::Object(Rc::new(Object::new(
            TYPE_SET,
            ObjectData::Set {
                items: RefCell::new(set),
            },
        ))))
    } else {
        Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("set() takes 0 or 1 argument ({} given)", args.len()),
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

