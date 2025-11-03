use super::super::type_def::{TypeDef, TypeFlags};

/// NoneType 타입 정의 등록
pub fn register_type() -> TypeDef {
    TypeDef::new("NoneType", TypeFlags::IMMUTABLE)
}

