use super::super::bytecode::Value;
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::type_name;

/// len() builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError { expected: 1, got: args.len() },
            format!("len() takes exactly 1 argument ({} given)", args.len())
        ));
    }

    let v = &args[0];
    match v {
        Value::Object(obj) => {
            use super::super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => Ok(Value::Int(s.chars().count() as i64)),
                _ => Err(err(
                    VmErrorKind::TypeError("len"),
                    format!("object of type '{}' has no len()", type_name(v))
                )),
            }
        }
        _ => Err(err(
            VmErrorKind::TypeError("len"),
            format!("object of type '{}' has no len()", type_name(v))
        )),
    }
}

