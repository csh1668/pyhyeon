use crate::vm::{Value, VmErrorKind, VmResult, builtins::bool_builtin, err};

/// assert builtin 함수
pub fn call(args: Vec<Value>) -> VmResult<Value> {
    if args.len() != 1 {
        return Err(err(
            VmErrorKind::ArityError {
                expected: 1,
                got: args.len(),
            },
            format!("assert() takes exactly 1 argument ({} given)", args.len()),
        ));
    }
    // let v = &args[0];
    let b = bool_builtin::to_bool(&args[0]);
    if !b {
        return Err(err(
            VmErrorKind::AssertionError,
            "AssertionError: assert failed".to_string(),
        ));
    }
    Ok(Value::None)
}
