use super::super::bytecode::Value;
use super::super::{VmError, VmErrorKind, VmResult, err};
use super::make_string;
use crate::runtime_io::RuntimeIo;

/// input() builtin 함수
pub fn call<IO: RuntimeIo>(args: Vec<Value>, io: &mut IO) -> VmResult<Value> {
    if args.len() > 1 {
        return Err(err(
            VmErrorKind::ArityError { expected: 1, got: args.len() },
            format!("input() takes at most 1 argument ({} given)", args.len())
        ));
    }

    let prompt = if !args.is_empty() {
        let prompt_val = &args[0];
        match prompt_val {
            Value::Object(obj) => {
                use crate::vm::value::ObjectData;
                if let ObjectData::String(s) = &obj.data {
                    Some(s.as_str())
                } else {
                    return Err(err(
                        VmErrorKind::TypeError("input"),
                        format!("input() argument must be str, not '{}'", super::type_name(prompt_val))
                    ));
                }
            }
            _ => {
                return Err(err(
                    VmErrorKind::TypeError("input"),
                    format!("input() argument must be str, not '{}'", super::type_name(prompt_val))
                ));
            }
        }
    } else {
        None
    };

    use crate::runtime_io::ReadResult;
    match io.read_line_with_prompt(prompt) {
        ReadResult::Ok(line) => Ok(make_string(line)),
        ReadResult::WaitingForInput => Err(err(
            VmErrorKind::TypeError("input"),
            "Waiting for input".into()
        )),
        ReadResult::Error(e) => Err(err(VmErrorKind::TypeError("io"), e)),
    }
}

