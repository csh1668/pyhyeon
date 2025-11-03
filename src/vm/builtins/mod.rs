pub mod print;
pub mod input;
pub mod int;
pub mod bool_builtin;
pub mod str_builtin;
pub mod len;
pub mod range;
pub mod none_type;
pub mod list;
pub mod dict;
pub mod list_methods;
pub mod dict_methods;

#[cfg(test)]
mod tests;

use super::bytecode::Value;
use super::type_def::{TypeDef, TYPE_INT, TYPE_BOOL, TYPE_STR, TYPE_NONE, TYPE_RANGE, TYPE_LIST, TYPE_DICT};
use super::{VmError, VmErrorKind, VmResult, err};
use crate::runtime_io::RuntimeIo;

// ========== 빌트인 함수 ID ==========
pub const BUILTIN_PRINT_ID: u8 = 0;
pub const BUILTIN_INPUT_ID: u8 = 1;
pub const BUILTIN_INT_ID: u8 = 2;
pub const BUILTIN_BOOL_ID: u8 = 3;
pub const BUILTIN_STR_ID: u8 = 4;
pub const BUILTIN_LEN_ID: u8 = 5;
pub const BUILTIN_RANGE_ID: u8 = 6;
pub const BUILTIN_LIST_ID: u8 = 7;
pub const BUILTIN_DICT_ID: u8 = 8;

// ========== Builtin 호출 디스패처 ==========
/// Builtin 함수 호출
pub fn call_builtin<IO: RuntimeIo>(
    id: u8,
    args: Vec<Value>,
    io: &mut IO,
) -> VmResult<Value> {
    match id {
        BUILTIN_PRINT_ID => print::call(args, io),
        BUILTIN_INPUT_ID => input::call(args, io),
        BUILTIN_INT_ID => int::call(args),
        BUILTIN_BOOL_ID => bool_builtin::call(args),
        BUILTIN_STR_ID => str_builtin::call(args),
        BUILTIN_LEN_ID => len::call(args),
        BUILTIN_RANGE_ID => range::create_range(args),
        _ => Err(err(
            VmErrorKind::TypeError("builtin"),
            format!("unknown builtin id {}", id)
        )),
    }
}

// ========== 헬퍼 함수들 ==========

// 유틸리티 함수들은 vm::utils에서 재export
pub use super::type_def::make_string;
pub(crate) use super::utils::{display_value, type_name};

