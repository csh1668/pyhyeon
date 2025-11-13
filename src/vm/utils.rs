//! VM 유틸리티 함수
//!
//! Value 표시, 타입 이름, 동등성 비교 등 VM에서 공통적으로 사용되는 헬퍼 함수들을 제공합니다.

use super::bytecode::Value;
use super::type_def::TYPE_USER_START;
use super::value::{BuiltinInstanceData, DictKey, Object, ObjectData};
use super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::{BuiltinClassType, TYPE_DICT, TYPE_LIST, TYPE_RANGE, TYPE_STR};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Value를 출력 가능한 문자열로 변환
///
/// Python의 `str()` 표현과 유사하게 값을 문자열로 변환합니다.
///
/// # Examples
///
/// ```ignore
/// display_value(&Value::Int(42))        // → "42"
/// display_value(&Value::Bool(true))     // → "True"
/// display_value(&Value::None)           // → "None"
/// ```
pub fn display_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Float(f) => f.to_string(),
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::None => "None".to_string(),
        Value::Object(obj) => match &obj.data {
            ObjectData::String(s) => s.clone(),
            ObjectData::List { items } => {
                let items_ref = items.borrow();
                let contents: Vec<String> = items_ref.iter().map(display_value).collect();
                format!("[{}]", contents.join(", "))
            }
            ObjectData::Dict { map } => {
                use super::value::DictKey;
                let map_ref = map.borrow();
                let contents: Vec<String> = map_ref
                    .iter()
                    .map(|(k, v)| {
                        let key_str = match k {
                            DictKey::Int(i) => i.to_string(),
                            DictKey::String(s) => format!("\"{}\"", s),
                            DictKey::Bool(b) => if *b { "True" } else { "False" }.to_string(),
                        };
                        format!("{}: {}", key_str, display_value(v))
                    })
                    .collect();
                format!("{{{}}}", contents.join(", "))
            }
            ObjectData::UserClass { class_id, .. } => format!("<class user_{}>", class_id),
            ObjectData::UserInstance { class_id } => {
                format!("<instance of user_{}>", class_id)
            }
            ObjectData::BuiltinClass { class_type } => {
                format!("<class '{}'>", class_type.name())
            }
            ObjectData::BuiltinInstance { class_type, .. } => {
                format!("<{} object>", class_type.name())
            }
            ObjectData::UserFunction { func_id, .. } => {
                format!("<function lambda#{}>", func_id)
            }
        },
    }
}

/// Value의 타입 이름 반환
///
/// Python의 `type(x).__name__`과 유사합니다.
///
/// # Examples
///
/// ```ignore
/// type_name(&Value::Int(42))        // → "int"
/// type_name(&Value::Bool(true))     // → "bool"
/// type_name(&Value::None)           // → "NoneType"
/// ```
pub fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Int(_) => "int",
        Value::Float(_) => "float",
        Value::Bool(_) => "bool",
        Value::None => "NoneType",
        Value::Object(obj) => match &obj.data {
            ObjectData::String(_) => "str",
            ObjectData::List { .. } => "list",
            ObjectData::Dict { .. } => "dict",
            ObjectData::UserClass { .. } => "type",
            ObjectData::UserInstance { .. } => "instance",
            ObjectData::BuiltinClass { .. } => "type",
            ObjectData::BuiltinInstance { class_type, .. } => class_type.name(),
            ObjectData::UserFunction { .. } => "function",
        },
    }
}

/// Value 동등성 비교
///
/// Python의 `==` 연산자 의미론을 구현합니다.
///
/// # Rules
///
/// - 동일 타입 primitive 값: 값 비교
/// - String 객체: 문자열 내용 비교
/// - 다른 객체: 포인터 비교 (identity)
/// - 서로 다른 타입: `false`
pub fn eq_vals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Float(x), Value::Float(y)) => x == y,
        (Value::Int(x), Value::Float(y)) => (*x as f64) == *y,
        (Value::Float(x), Value::Int(y)) => *x == (*y as f64),
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::None, Value::None) => true,
        (Value::Object(x), Value::Object(y)) => {
            if Rc::ptr_eq(x, y) {
                return true;
            }
            match (&x.data, &y.data) {
                (ObjectData::String(s1), ObjectData::String(s2)) => s1 == s2,
                _ => false,
            }
        }
        _ => false,
    }
}

// ========== Object 생성 헬퍼 함수들 (make_*) ==========

/// String 객체 생성
pub fn make_string(s: String) -> Value {
    Value::Object(Rc::new(Object::new(TYPE_STR, ObjectData::String(s))))
}

/// List 객체 생성
pub fn make_list(items: Vec<Value>) -> Value {
    Value::Object(Rc::new(Object::new(
        TYPE_LIST,
        ObjectData::List {
            items: RefCell::new(items),
        },
    )))
}

/// Range 객체 생성
pub fn make_range(current: i64, stop: i64, step: i64) -> Value {
    Value::Object(Rc::new(Object::new(
        TYPE_RANGE,
        ObjectData::BuiltinInstance {
            class_type: BuiltinClassType::Range,
            data: BuiltinInstanceData::Range {
                current: RefCell::new(current),
                stop,
                step,
            },
        },
    )))
}

/// Dict 객체 생성
pub fn make_dict(map: HashMap<DictKey, Value>) -> Value {
    Value::Object(Rc::new(Object::new(
        TYPE_DICT,
        ObjectData::Dict {
            map: RefCell::new(map),
        },
    )))
}

/// 사용자 정의 클래스 객체 생성
pub fn make_user_class(class_id: u16, methods: HashMap<String, u16>) -> Value {
    Value::Object(Rc::new(Object::new(
        TYPE_USER_START + class_id,
        ObjectData::UserClass { class_id, methods },
    )))
}

/// 사용자 정의 인스턴스 객체 생성
pub fn make_user_instance(class_id: u16) -> Value {
    Value::Object(Rc::new(Object::new_with_attrs(
        TYPE_USER_START + class_id,
        ObjectData::UserInstance { class_id },
    )))
}

/// Builtin 클래스 객체 생성
pub fn make_builtin_class(class_type: BuiltinClassType) -> Value {
    let type_id = match class_type {
        BuiltinClassType::Range => TYPE_RANGE,
        BuiltinClassType::List => TYPE_LIST,
        BuiltinClassType::Dict => TYPE_DICT,
    };
    Value::Object(Rc::new(Object::new(
        type_id,
        ObjectData::BuiltinClass { class_type },
    )))
}

// ========== 타입 추출 헬퍼 함수들 (expect_*) ==========

/// Value에서 int 추출
pub fn expect_int(v: &Value) -> VmResult<i64> {
    match v {
        Value::Int(n) => Ok(*n),
        _ => Err(err(
            VmErrorKind::TypeError("int"),
            format!("expected int, got {}", type_name(v)),
        )),
    }
}

/// Value에서 float 추출
pub fn expect_float(v: &Value) -> VmResult<f64> {
    match v {
        Value::Float(f) => Ok(*f),
        _ => Err(err(
            VmErrorKind::TypeError("float"),
            format!("expected float, got {}", type_name(v)),
        )),
    }
}

/// Value에서 String 데이터 추출
pub fn expect_string(v: &Value) -> VmResult<&str> {
    match v {
        Value::Object(obj) => match &obj.data {
            ObjectData::String(s) => Ok(s.as_str()),
            _ => Err(err(
                VmErrorKind::TypeError("str"),
                "expected string object".into(),
            )),
        },
        _ => Err(err(VmErrorKind::TypeError("str"), "expected String".into())),
    }
}

/// Value에서 bool 추출
pub fn expect_bool(v: &Value) -> VmResult<bool> {
    match v {
        Value::Bool(b) => Ok(*b),
        _ => Err(err(
            VmErrorKind::TypeError("bool"),
            format!("expected bool, got {}", type_name(v)),
        )),
    }
}

/// Value에서 List 데이터 추출 (borrowed)
pub fn expect_list(v: &Value) -> VmResult<Vec<Value>> {
    match v {
        Value::Object(obj) => match &obj.data {
            ObjectData::List { items } => Ok(items.borrow().clone()),
            _ => Err(err(
                VmErrorKind::TypeError("list"),
                "expected list object".into(),
            )),
        },
        _ => Err(err(VmErrorKind::TypeError("list"), "expected List".into())),
    }
}
