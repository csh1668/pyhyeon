use super::super::bytecode::Value;
use super::super::type_def::{TypeDef, TypeFlags};
use super::super::utils::expect_int;
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::{TYPE_INT, type_name};

/// int() builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("int() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    let v = &args[0];
    match v {
        Value::Int(i) => Ok(Value::Int(*i)),
        Value::Float(f) => Ok(Value::Int(*f as i64)),
        Value::Bool(b) => Ok(Value::Int(if *b { 1 } else { 0 })),
        Value::Object(obj) => {
            use super::super::value::ObjectData;
            match &obj.data {
                ObjectData::String(s) => s.trim().parse::<i64>().map(Value::Int).map_err(|_| {
                    err(
                        VmErrorKind::TypeError("int"),
                        format!("invalid literal for int() with base 10: '{}'", s),
                    )
                }),
                _ => Err(err(
                    VmErrorKind::TypeError("int"),
                    format!(
                        "int() argument must be a string or a number, not '{}'",
                        type_name(v)
                    ),
                )),
            }
        }
        Value::None => Err(err(
            VmErrorKind::TypeError("int"),
            "int() argument must be a string or a number, not 'NoneType'".into(),
        )),
    }
}

pub fn register_type() -> TypeDef {
    TypeDef::new("int", TypeFlags::IMMUTABLE)
}

// ========== 매직 메서드 구현 ==========

/// __add__: Int + Int
pub fn int_add(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Int(a.wrapping_add(b)))
}

/// __sub__: Int - Int
pub fn int_sub(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Int(a.wrapping_sub(b)))
}

/// __mul__: Int * Int
pub fn int_mul(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Int(a.wrapping_mul(b)))
}

/// __floordiv__: Int // Int
pub fn int_floordiv(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    if b == 0 {
        return Err(err(
            VmErrorKind::ZeroDivision,
            "integer division or modulo by zero".into(),
        ));
    }
    Ok(Value::Int(a / b))
}

/// __truediv__: Int / Int
pub fn int_truediv(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    if b == 0 {
        return Err(err(
            VmErrorKind::ZeroDivision,
            "integer division or modulo by zero".into(),
        ));
    }
    Ok(Value::Float(a as f64 / b as f64))
}

/// __mod__: Int % Int
pub fn int_mod(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    if b == 0 {
        return Err(err(
            VmErrorKind::ZeroDivision,
            "integer division or modulo by zero".into(),
        ));
    }
    Ok(Value::Int(a % b))
}

/// __neg__: -Int
pub fn int_neg(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    Ok(Value::Int(-a))
}

/// __pos__: +Int
pub fn int_pos(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    Ok(Value::Int(a))
}

/// __lt__: Int < Int
pub fn int_lt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a < b))
}

/// __le__: Int <= Int
pub fn int_le(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a <= b))
}

/// __gt__: Int > Int
pub fn int_gt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a > b))
}

/// __ge__: Int >= Int
pub fn int_ge(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a >= b))
}

/// __eq__: Int == Int
pub fn int_eq(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a == b))
}

/// __ne__: Int != Int
pub fn int_ne(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let a = expect_int(receiver)?;
    let b = expect_int(&args[0])?;
    Ok(Value::Bool(a != b))
}
