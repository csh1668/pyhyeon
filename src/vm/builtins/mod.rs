pub mod print;
pub mod input;
pub mod int;
pub mod bool_builtin;
pub mod str_builtin;
pub mod len;
pub mod range;
pub mod none_type;

#[cfg(test)]
mod tests;

use super::bytecode::Value;
use super::type_def::{TypeDef, TYPE_INT, TYPE_BOOL, TYPE_STR, TYPE_NONE, TYPE_RANGE};
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

// ========== 타입 등록 ==========
/// 모든 builtin 타입 정의 반환
/// 
/// 타입 ID는 벡터의 인덱스와 일치해야 합니다:
/// - Index 0 = TYPE_INT (0)
/// - Index 1 = TYPE_BOOL (1)
/// - Index 2 = TYPE_STR (2)
/// - Index 3 = TYPE_NONE (3)
/// - Index 4 = TYPE_RANGE (4)
pub fn register_all_types() -> Vec<TypeDef> {
    vec![
        int::register_type(),           // TYPE_INT = 0
        bool_builtin::register_type(),  // TYPE_BOOL = 1
        str_builtin::register_type(),   // TYPE_STR = 2
        none_type::register_type(),     // TYPE_NONE = 3
        range::register_type(),         // TYPE_RANGE = 4
    ]
}

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

