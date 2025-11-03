use super::super::bytecode::Value;
use super::super::type_def::{TypeDef, TypeFlags};
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::{type_name, TYPE_INT};

/// int() builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError { expected: 1, got: args.len() },
            format!("int() takes exactly 1 argument ({} given)", args.len())
        ));
    }

    let v = &args[0];
    match v {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        Value::Object(obj) => {
            use super::super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => {
                    s.trim()
                        .parse::<i64>()
                        .map(Value::Int)
                        .map_err(|_| err(
                            VmErrorKind::TypeError("int"),
                            format!("invalid literal for int() with base 10: '{}'", s)
                        ))
                }
                _ => Err(err(
                    VmErrorKind::TypeError("int"),
                    format!("int() argument must be a string or a number, not '{}'", type_name(v))
                )),
            }
        }
        Value::None => Err(err(
            VmErrorKind::TypeError("int"),
            "int() argument must be a string or a number, not 'NoneType'".into()
        )),
    }
}

pub fn register_type() -> TypeDef {
    TypeDef::new("int", TypeFlags::IMMUTABLE)
}

