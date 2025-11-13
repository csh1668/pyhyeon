use super::super::bytecode::Value;
use super::super::type_def::{Arity, MethodImpl, NativeMethod, TypeDef, TypeFlags};
use super::super::utils::{expect_list, expect_string, make_list, make_string};
use super::super::value::ObjectData;
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::display_value;
use crate::builtins::TYPE_STR;

/// str() builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    // 인자 개수 검증
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str() takes exactly 1 argument ({} given)", args.len()),
        ));
    }

    let v = &args[0];
    Ok(make_string(display_value(v)))
}

// ========== String 메서드 구현들 ==========

pub fn str_upper(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 0,
                got: args.len(),
            },
            format!("str.upper() takes 0 arguments but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.to_uppercase()))
}

pub fn str_lower(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 0,
                got: args.len(),
            },
            format!("str.lower() takes 0 arguments but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.to_lowercase()))
}

pub fn str_strip(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 0,
                got: args.len(),
            },
            format!("str.strip() takes 0 arguments but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    Ok(make_string(s.trim().to_string()))
}

pub fn str_split(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() > 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!(
                "str.split() takes 0 or 1 argument, but {} given",
                args.len()
            ),
        ));
    }
    let sep = match args.get(0) {
        None => " ",
        Some(value) if expect_string(value).is_ok() => expect_string(value)?,
        _ => {
            return Err(err(
                VmErrorKind::TypeError("str.split"),
                format!(
                    "str.split() takes 0 or 1 argument, but {} given",
                    args.len()
                ),
            ));
        }
    };
    let s = expect_string(receiver)?;
    let result = s
        .split(sep)
        .map(|s| make_string(s.to_string()))
        .collect::<Vec<Value>>();
    Ok(make_list(result))
}

pub fn str_join(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str.join() takes 1 argument but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let list = expect_list(&args[0])?;
    let strings: Vec<String> = list
        .iter()
        .map(|v| expect_string(v).map(|s| s.to_string()))
        .collect::<Result<Vec<String>, _>>()?;
    let result = strings.join(s);
    Ok(make_string(result))
}

pub fn str_replace(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 2 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 2,
                got: args.len(),
            },
            format!("str.replace() takes 2 arguments but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let old = expect_string(&args[0])?;
    let new = expect_string(&args[1])?;
    Ok(make_string(s.replace(old, new)))
}

pub fn str_starts_with(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str.startswith() takes 1 argument but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let prefix = expect_string(&args[0])?;
    Ok(Value::Bool(s.starts_with(prefix)))
}

pub fn str_ends_with(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str.endswith() takes 1 argument but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let suffix = expect_string(&args[0])?;
    Ok(Value::Bool(s.ends_with(suffix)))
}

pub fn str_find(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str.find() takes 1 argument but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let substr = expect_string(&args[0])?;
    match s.find(substr) {
        Some(pos) => Ok(Value::Int(pos as i64)),
        None => Ok(Value::Int(-1)),
    }
}

pub fn str_count(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("str.count() takes 1 argument but {} given", args.len()),
        ));
    }
    let s = expect_string(receiver)?;
    let substr = expect_string(&args[0])?;
    let count = s.matches(substr).count();
    Ok(Value::Int(count as i64))
}

// ========== 매직 메서드 구현 ==========

/// __add__: String + String (concatenation)
pub fn str_add(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(make_string(s1.to_string() + s2))
}

/// __mul__: String * Int (repetition)
pub fn str_mul(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s = expect_string(receiver)?;
    let n = match &args[0] {
        Value::Int(n) => *n,
        _ => {
            return Err(err(
                VmErrorKind::TypeError("str"),
                "can't multiply string by non-int".into(),
            ));
        }
    };

    if n < 0 {
        Ok(make_string(String::new()))
    } else {
        Ok(make_string(s.repeat(n as usize)))
    }
}

/// __lt__: String < String
pub fn str_lt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 < s2))
}

/// __le__: String <= String
pub fn str_le(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 <= s2))
}

/// __gt__: String > String
pub fn str_gt(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 > s2))
}

/// __ge__: String >= String
pub fn str_ge(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 >= s2))
}

/// __eq__: String == String
pub fn str_eq(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 == s2))
}

/// __ne__: String != String
pub fn str_ne(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    let s1 = expect_string(receiver)?;
    let s2 = expect_string(&args[0])?;
    Ok(Value::Bool(s1 != s2))
}

/// str 타입 정의 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("str", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE).with_methods(vec![
        (
            "upper",
            MethodImpl::Native {
                func: NativeMethod::StrUpper,
                arity: Arity::Exact(0),
            },
        ),
        (
            "lower",
            MethodImpl::Native {
                func: NativeMethod::StrLower,
                arity: Arity::Exact(0),
            },
        ),
        (
            "strip",
            MethodImpl::Native {
                func: NativeMethod::StrStrip,
                arity: Arity::Exact(0),
            },
        ),
        (
            "split",
            MethodImpl::Native {
                func: NativeMethod::StrSplit,
                arity: Arity::Range(0, 1),
            },
        ),
        (
            "join",
            MethodImpl::Native {
                func: NativeMethod::StrJoin,
                arity: Arity::Exact(1),
            },
        ),
        (
            "replace",
            MethodImpl::Native {
                func: NativeMethod::StrReplace,
                arity: Arity::Exact(2),
            },
        ),
        (
            "startswith",
            MethodImpl::Native {
                func: NativeMethod::StrStartsWith,
                arity: Arity::Exact(1),
            },
        ),
        (
            "endswith",
            MethodImpl::Native {
                func: NativeMethod::StrEndsWith,
                arity: Arity::Exact(1),
            },
        ),
        (
            "find",
            MethodImpl::Native {
                func: NativeMethod::StrFind,
                arity: Arity::Exact(1),
            },
        ),
        (
            "count",
            MethodImpl::Native {
                func: NativeMethod::StrCount,
                arity: Arity::Exact(1),
            },
        ),
    ])
}
