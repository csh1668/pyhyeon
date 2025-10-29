use super::type_def::{NativeMethod, TYPE_STR};
use super::value::{BuiltinInstanceData, Object, ObjectData};
use crate::vm::bytecode::Value;
use std::rc::Rc;

pub type NativeResult = Result<Value, NativeError>;

#[derive(Debug, Clone)]
pub struct NativeError {
    pub message: String,
}

impl NativeError {
    /// 새 에러 생성
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// 타입 에러 생성 헬퍼
    pub fn type_error(expected: &str, got: &str) -> Self {
        Self::new(format!("expected {}, got {}", expected, got))
    }

    /// Arity 에러 생성 헬퍼
    pub fn arity_error(method: &str, expected: usize, got: usize) -> Self {
        Self::new(format!(
            "{} takes {} argument(s) but {} given",
            method, expected, got
        ))
    }
}

/// Native 메서드 호출 인터페이스
///
/// 이 함수는 NativeMethod enum을 받아서 적절한 구현 함수를 호출합니다.
pub fn call_native_method(
    method: NativeMethod,
    receiver: &Value,
    args: Vec<Value>,
) -> NativeResult {
    match method {
        NativeMethod::StrUpper => str_upper(receiver, args),
        NativeMethod::StrLower => str_lower(receiver, args),
        NativeMethod::StrStrip => str_strip(receiver, args),
        NativeMethod::StrSplit => str_split(receiver, args),
        NativeMethod::StrJoin => str_join(receiver, args),
        NativeMethod::StrReplace => str_replace(receiver, args),
        NativeMethod::StrStartsWith => str_starts_with(receiver, args),
        NativeMethod::StrEndsWith => str_ends_with(receiver, args),
        NativeMethod::StrFind => str_find(receiver, args),
        NativeMethod::StrCount => str_count(receiver, args),
        NativeMethod::RangeIter => range_iter(receiver, args),
        NativeMethod::RangeHasNext => range_has_next(receiver, args),
        NativeMethod::RangeNext => range_next(receiver, args),
    }
}

// ========== 헬퍼 함수들 ==========

/// Value에서 String 데이터 추출
fn expect_string(v: &Value) -> Result<&str, NativeError> {
    match v {
        Value::Object(obj) => match &obj.data {
            ObjectData::String(s) => Ok(s.as_str()),
            _ => Err(NativeError::type_error("string object", "other object")),
        },
        _ => Err(NativeError::type_error("String", "other type")),
    }
}

/// String Object 생성
fn make_string(s: String) -> Value {
    Value::Object(Rc::new(Object::new(TYPE_STR, ObjectData::String(s))))
}

// ========== String 메서드 구현들 ==========

fn str_upper(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if !args.is_empty() {
        return Err(NativeError::arity_error("str.upper()", 0, args.len()));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.to_uppercase()))
}

fn str_lower(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if !args.is_empty() {
        return Err(NativeError::arity_error("str.lower()", 0, args.len()));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.to_lowercase()))
}

fn str_strip(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if !args.is_empty() {
        return Err(NativeError::arity_error("str.strip()", 0, args.len()));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.trim().to_string()))
}

fn str_split(_receiver: &Value, _args: Vec<Value>) -> NativeResult {
    // split()은 리스트를 반환해야 하므로 list 타입이 필요
    Err(NativeError::new(
        "str.split() not implemented yet (requires list type)",
    ))
}

fn str_join(_receiver: &Value, _args: Vec<Value>) -> NativeResult {
    // join()은 리스트를 인자로 받으므로 list 타입이 필요
    Err(NativeError::new(
        "str.join() not implemented yet (requires list type)",
    ))
}

fn str_replace(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if args.len() != 2 {
        return Err(NativeError::arity_error("str.replace()", 2, args.len()));
    }
    let s = expect_string(receiver)?;
    let old = expect_string(&args[0])?;
    let new = expect_string(&args[1])?;
    Ok(make_string(s.replace(old, new)))
}

fn str_starts_with(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if args.len() != 1 {
        return Err(NativeError::arity_error("str.startswith()", 1, args.len()));
    }
    let s = expect_string(receiver)?;
    let prefix = expect_string(&args[0])?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

fn str_ends_with(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if args.len() != 1 {
        return Err(NativeError::arity_error("str.endswith()", 1, args.len()));
    }
    let s = expect_string(receiver)?;
    let suffix = expect_string(&args[0])?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

fn str_find(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if args.len() != 1 {
        return Err(NativeError::arity_error("str.find()", 1, args.len()));
    }
    let s = expect_string(receiver)?;
    let substr = expect_string(&args[0])?;
    match s.find(substr) {
        Some(pos) => Ok(Value::Int(pos as i64)),
        None => Ok(Value::Int(-1)),
    }
}

fn str_count(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if args.len() != 1 {
        return Err(NativeError::arity_error("str.count()", 1, args.len()));
    }
    let s = expect_string(receiver)?;
    let substr = expect_string(&args[0])?;
    let count = s.matches(substr).count();
    Ok(Value::Int(count as i64))
}

// ========== Range 메서드 구현들 ==========

fn range_iter(receiver: &Value, _args: Vec<Value>) -> NativeResult {
    // Range는 자기 자신이 iterator (Python과 동일)
    Ok(receiver.clone())
}

fn range_has_next(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if !args.is_empty() {
        return Err(NativeError::arity_error("range.__has_next__()", 0, args.len()));
    }
    
    match receiver {
        Value::Object(obj) => match &obj.data {
            ObjectData::BuiltinInstance {
                data: BuiltinInstanceData::Range { current, stop, step },
                ..
            } => {
                let curr = *current.borrow();
                let has_next = if *step > 0 {
                    curr < *stop
                } else if *step < 0 {
                    curr > *stop
                } else {
                    false // step == 0은 에러이지만, 일단 false 반환
                };
                Ok(Value::Bool(has_next))
            }
            _ => Err(NativeError::type_error("Range", "other object")),
        },
        _ => Err(NativeError::type_error("Range", "other type")),
    }
}

fn range_next(receiver: &Value, args: Vec<Value>) -> NativeResult {
    if !args.is_empty() {
        return Err(NativeError::arity_error("range.__next__()", 0, args.len()));
    }
    
    match receiver {
        Value::Object(obj) => match &obj.data {
            ObjectData::BuiltinInstance {
                data: BuiltinInstanceData::Range { current, stop, step },
                ..
            } => {
                let mut curr_mut = current.borrow_mut();
                let value = *curr_mut;
                *curr_mut += *step;
                Ok(Value::Int(value))
            }
            _ => Err(NativeError::type_error("Range", "other object")),
        },
        _ => Err(NativeError::type_error("Range", "other type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::type_def::TYPE_STR;
    use crate::vm::value::ObjectData;
    use std::rc::Rc;

    #[test]
    fn test_str_upper() {
        let obj = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("hello".to_string()),
        )));

        let result = call_native_method(NativeMethod::StrUpper, &obj, vec![]);
        assert!(result.is_ok());

        if let Ok(Value::Object(obj)) = result {
            if let ObjectData::String(s) = &obj.data {
                assert_eq!(s, "HELLO");
            }
        }
    }

    #[test]
    fn test_str_lower() {
        let obj = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("WORLD".to_string()),
        )));

        let result = call_native_method(NativeMethod::StrLower, &obj, vec![]);
        assert!(result.is_ok());

        if let Ok(Value::Object(obj)) = result {
            if let ObjectData::String(s) = &obj.data {
                assert_eq!(s, "world");
            }
        }
    }

    #[test]
    fn test_str_replace() {
        let obj = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("hello world".to_string()),
        )));

        let old = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("world".to_string()),
        )));
        let new = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("rust".to_string()),
        )));

        let result = call_native_method(NativeMethod::StrReplace, &obj, vec![old, new]);
        assert!(result.is_ok());

        if let Ok(Value::Object(obj)) = result {
            if let ObjectData::String(s) = &obj.data {
                assert_eq!(s, "hello rust");
            }
        }
    }

    #[test]
    fn test_str_find() {
        let obj = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("hello world".to_string()),
        )));

        let substr = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("world".to_string()),
        )));

        let result = call_native_method(NativeMethod::StrFind, &obj, vec![substr]);
        assert!(result.is_ok());
        assert!(matches!(result.unwrap(), Value::Int(6)));
    }

    #[test]
    fn test_arity_error() {
        let obj = Value::Object(Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("test".to_string()),
        )));

        // upper()는 인자를 받지 않음
        let result = call_native_method(NativeMethod::StrUpper, &obj, vec![Value::Int(1)]);
        assert!(result.is_err());
    }
}
