pub mod assert_builtin;
pub mod bool_builtin;
pub mod dict;
pub mod dict_methods;
pub mod float;
pub mod input;
pub mod int;
pub mod len;
pub mod list;
pub mod list_methods;
pub mod none_type;
pub mod print;
pub mod range;
pub mod str_builtin;

#[cfg(test)]
mod tests;

use super::bytecode::Value;
use super::type_def::TypeDef;
use super::{VmError, VmErrorKind, VmResult, err};
use crate::builtins::{
    BUILTIN_ASSERT_ID, BUILTIN_BOOL_ID, BUILTIN_DICT_ID, BUILTIN_FLOAT_ID, BUILTIN_INPUT_ID,
    BUILTIN_INT_ID, BUILTIN_LEN_ID, BUILTIN_LIST_ID, BUILTIN_PRINT_ID, BUILTIN_RANGE_ID,
    BUILTIN_STR_ID,
};
use crate::runtime_io::RuntimeIo;

// ========== Builtin 호출 디스패처 ==========
/// Builtin 함수 호출
pub fn call_builtin<IO: RuntimeIo>(id: u8, args: Vec<Value>, io: &mut IO) -> VmResult<Value> {
    match id {
        BUILTIN_PRINT_ID => print::call(args, io),
        BUILTIN_INPUT_ID => input::call(args, io),
        BUILTIN_INT_ID => int::call(args),
        BUILTIN_BOOL_ID => bool_builtin::call(args),
        BUILTIN_STR_ID => str_builtin::call(args),
        BUILTIN_LEN_ID => len::call(args),
        BUILTIN_RANGE_ID => range::create_range(args),
        BUILTIN_FLOAT_ID => float::call(args),
        // TODO: list()와 dict() 생성자는 나중에 구현
        // BUILTIN_LIST_ID => list::call(args),
        // BUILTIN_DICT_ID => dict::call(args),
        BUILTIN_ASSERT_ID => assert_builtin::call(args),
        _ => Err(err(
            VmErrorKind::TypeError("builtin"),
            format!("unknown builtin id {}", id),
        )),
    }
}

// ========== 헬퍼 함수들 ==========

// 유틸리티 함수들은 vm::utils에서 재export
pub use super::utils::{display_value, make_string, type_name};
