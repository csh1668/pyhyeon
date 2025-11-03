use super::super::bytecode::Value;
use super::super::VmResult;
use super::display_value;
use crate::runtime_io::RuntimeIo;

/// print() builtin 함수
pub fn call<IO: RuntimeIo>(args: Vec<Value>, io: &mut IO) -> VmResult<Value> {
    if args.is_empty() {
        io.write_line("");
    } else {
        let parts: Vec<String> = args.iter().map(display_value).collect();
        let line = parts.join(" ");
        io.write_line(&line);
    }
    Ok(Value::None)
}
