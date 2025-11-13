use super::super::bytecode::Value;
use super::super::type_def::{TypeDef, TypeFlags};
use super::super::utils::expect_float;
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::type_name;

/// float() builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("float() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    let arg = &args[0];
    match arg {
        Value::Int(i) => Ok(Value::Float(*i as f64)),
        Value::Float(f) => Ok(Value::Float(*f)),
        Value::Bool(b) => Ok(Value::Float(if *b { 1.0 } else { 0.0 })),
        Value::Object(obj) => {
            // String 객체를 Float로 변환 시도
            use super::super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => s.trim().parse::<f64>().map(Value::Float).map_err(|_| {
                    err(
                        VmErrorKind::TypeError("float"),
                        format!("could not convert string to float: '{}'", s),
                    )
                }),
                _ => Err(err(
                    VmErrorKind::TypeError("float"),
                    format!(
                        "float() argument must be a string or a number, not '{}'",
                        type_name(arg)
                    ),
                )),
            }
        }
        Value::None => Err(err(
            VmErrorKind::TypeError("float"),
            "float() argument must be a string or a number, not 'NoneType'".to_string(),
        )),
    }
}

pub fn register_type() -> TypeDef {
    TypeDef::new("float", TypeFlags::IMMUTABLE)
}

// ========== 매직 메서드 구현 ==========

pub fn float_add(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Float(a + b))
}

pub fn float_sub(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Float(a - b))
}

pub fn float_mul(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Float(a * b))
}

pub fn float_true_div(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Float(a / b))
}

pub fn float_floor_div(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    if b == 0.0 {
        return Err(err(VmErrorKind::ZeroDivision, "division by zero".into()));
    }
    Ok(Value::Float((a / b).floor()))
}

pub fn float_mod(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    if b == 0.0 {
        return Err(err(VmErrorKind::ZeroDivision, "modulo by zero".into()));
    }
    Ok(Value::Float(a % b))
}

pub fn float_neg(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    Ok(Value::Float(-expect_float(receiver)?))
}

pub fn float_pos(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    Ok(Value::Float(expect_float(receiver)?))
}

pub fn float_lt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a < b))
}

pub fn float_le(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a <= b))
}

pub fn float_gt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a > b))
}

pub fn float_ge(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a >= b))
}

pub fn float_eq(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a == b))
}

pub fn float_ne(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let (a, b) = (expect_float(receiver)?, expect_float(&args[0])?);
    Ok(Value::Bool(a != b))
}
