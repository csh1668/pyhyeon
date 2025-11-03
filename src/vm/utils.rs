//! VM 유틸리티 함수
//!
//! Value 표시, 타입 이름, 동등성 비교 등 VM에서 공통적으로 사용되는 헬퍼 함수들을 제공합니다.

use super::bytecode::Value;
use super::value::ObjectData;
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
        Value::Bool(b) => if *b { "True" } else { "False" }.to_string(),
        Value::None => "None".to_string(),
        Value::Object(obj) => match &obj.data {
            ObjectData::String(s) => s.clone(),
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
        Value::Bool(_) => "bool",
        Value::None => "NoneType",
        Value::Object(obj) => match &obj.data {
            ObjectData::String(_) => "str",
            ObjectData::UserClass { .. } => "type",
            ObjectData::UserInstance { .. } => "instance",
            ObjectData::BuiltinClass { .. } => "type",
            ObjectData::BuiltinInstance { class_type, .. } => class_type.name(),
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

