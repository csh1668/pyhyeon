use super::super::bytecode::Value;
use super::super::type_def::{TypeDef, TypeFlags};
use super::super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::TYPE_BOOL;

/// bool() builtin 함수
///
/// Truthy/Falsy 규칙:
/// - False, 0, None, "" → False
/// - 나머지 → True
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    // 인자 개수 검증
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("bool() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    let v = &args[0];
    let result = to_bool(v);

    Ok(Value::Bool(result))
}

pub fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Int(i) => *i != 0,
        Value::Float(f) => *f != 0.0,
        Value::Object(obj) => {
            use super::super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => !s.is_empty(),
                _ => true,
            }
        }
        Value::None => false,
    }
}

/// bool 타입 정의 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("bool", TypeFlags::IMMUTABLE)
}
