use super::super::bytecode::Value;
use super::super::type_def::{Arity, MethodImpl, NativeMethod, TypeDef, TypeFlags};
use super::super::utils::make_range;
use super::super::value::{BuiltinInstanceData, Object, ObjectData};
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::type_name;
use crate::builtins::TYPE_RANGE;

/// range() 생성자
///
/// range(stop)
/// range(start, stop)
/// range(start, stop, step)
///
/// ```python
/// for i in range(5):        # 0, 1, 2, 3, 4
///     print(i)
///
/// for i in range(2, 5):     # 2, 3, 4
///     print(i)
///
/// for i in range(0, 10, 2): # 0, 2, 4, 6, 8
///     print(i)
/// ```
pub fn create_range(args: Vec<Value>) -> VmResult<Value> {
    // Note: Arity is usually validated by semantic analyzer (1-3 args)
    // However, we still check at runtime for direct calls (e.g., in tests or dynamic scenarios)
    let (start, stop, step) = match args.len() {
        1 => {
            // range(stop)
            let stop = extract_int(&args[0])?;
            (0, stop, 1)
        }
        2 => {
            // range(start, stop)
            let start = extract_int(&args[0])?;
            let stop = extract_int(&args[1])?;
            (start, stop, 1)
        }
        3 => {
            // range(start, stop, step)
            let start = extract_int(&args[0])?;
            let stop = extract_int(&args[1])?;
            let step = extract_int(&args[2])?;

            if step == 0 {
                return Err(err(
                    VmErrorKind::TypeError("range"),
                    "range() step argument must not be zero".into(),
                ));
            }

            (start, stop, step)
        }
        n => {
            // Runtime arity check for direct calls (e.g., tests, dynamic code)
            return Err(err(
                VmErrorKind::ArityError {
                    expected: 3,
                    got: n,
                },
                format!("range() takes 1 to 3 arguments ({} given)", n),
            ));
        }
    };

    Ok(make_range(start, stop, step))
}

/// Value에서 int 추출
fn extract_int(v: &Value) -> VmResult<i64> {
    match v {
        Value::Int(i) => Ok(*i),
        _ => Err(err(
            VmErrorKind::TypeError("int"),
            format!("range() argument must be int, not '{}'", type_name(v)),
        )),
    }
}

// ========== Range 메서드 구현들 ==========

pub fn range_iter(receiver: &Value, _args: Vec<Value>) -> VmResult<Value> {
    // Range는 자기 자신이 iterator (Python과 동일)
    Ok(receiver.clone())
}

pub fn range_has_next(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 0,
                got: args.len(),
            },
            format!(
                "range.__has_next__() takes 0 arguments but {} given",
                args.len()
            ),
        ));
    }

    match receiver {
        Value::Object(obj) => match &obj.data {
            ObjectData::BuiltinInstance {
                data:
                    BuiltinInstanceData::Range {
                        current,
                        stop,
                        step,
                    },
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
            _ => Err(err(
                VmErrorKind::TypeError("range"),
                "expected Range object".into(),
            )),
        },
        _ => Err(err(
            VmErrorKind::TypeError("range"),
            "expected Range".into(),
        )),
    }
}

pub fn range_next(receiver: &Value, args: Vec<Value>) -> VmResult<Value> {
    if !args.is_empty() {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 0,
                got: args.len(),
            },
            format!(
                "range.__next__() takes 0 arguments but {} given",
                args.len()
            ),
        ));
    }

    match receiver {
        Value::Object(obj) => match &obj.data {
            ObjectData::BuiltinInstance {
                data:
                    BuiltinInstanceData::Range {
                        current,
                        stop,
                        step,
                    },
                ..
            } => {
                let mut curr_mut = current.borrow_mut();
                let value = *curr_mut;
                *curr_mut += *step;
                Ok(Value::Int(value))
            }
            _ => Err(err(
                VmErrorKind::TypeError("range"),
                "expected Range object".into(),
            )),
        },
        _ => Err(err(
            VmErrorKind::TypeError("range"),
            "expected Range".into(),
        )),
    }
}

/// range 타입 정의 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("range", TypeFlags::IMMUTABLE | TypeFlags::ITERABLE).with_methods(vec![
        (
            "__iter__",
            MethodImpl::Native {
                func: NativeMethod::RangeIter,
                arity: Arity::Exact(0),
            },
        ),
        (
            "__has_next__",
            MethodImpl::Native {
                func: NativeMethod::RangeHasNext,
                arity: Arity::Exact(0),
            },
        ),
        (
            "__next__",
            MethodImpl::Native {
                func: NativeMethod::RangeNext,
                arity: Arity::Exact(0),
            },
        ),
    ])
}
