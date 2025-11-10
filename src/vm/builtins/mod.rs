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
use super::type_def::{
    TYPE_BOOL, TYPE_DICT, TYPE_FLOAT, TYPE_INT, TYPE_LIST, TYPE_NONE, TYPE_RANGE, TYPE_STR, TypeDef,
};
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
pub const BUILTIN_FLOAT_ID: u8 = 7;
pub const BUILTIN_LIST_ID: u8 = 8;
pub const BUILTIN_DICT_ID: u8 = 9;

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
        _ => Err(err(
            VmErrorKind::TypeError("builtin"),
            format!("unknown builtin id {}", id),
        )),
    }
}

// ========== 헬퍼 함수들 ==========

// 유틸리티 함수들은 vm::utils에서 재export
pub use super::utils::{make_string, display_value, type_name};
