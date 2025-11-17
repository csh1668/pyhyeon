//! # 설계 원칙
//!
//! 1. **모든 것은 객체다**: String도 Object, 클래스도 Object
//! 2. **타입 테이블 기반**: 각 Object는 `type_id`로 타입을 참조
//! 3. **속성 지연 할당**: 필요한 경우에만 `attributes` HashMap 할당 (메모리 최적화)

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::builtins::BuiltinClassType;

/// 통일된 런타임 객체
///
/// Python의 PyObject에 해당합니다. 모든 heap-allocated 값은 이 구조체로 표현됩니다.
///
/// # 메모리 레이아웃
///
/// - `type_id`: 2 bytes (타입 테이블 인덱스)
/// - `data`: 8-24 bytes (ObjectData enum)
/// - `attributes`: 16 bytes (Option<RefCell<HashMap>>)
///
/// 총 ~26-42 bytes + heap data
#[derive(Debug, Clone)]
pub struct Object {
    /// 타입 ID (Module.types 테이블의 인덱스)
    /// 예: TYPE_STR (2), TYPE_RANGE (4), 또는 사용자 정의 타입 (100+)
    pub type_id: u16,

    /// 객체의 실제 데이터
    pub data: ObjectData,

    /// 인스턴스 속성 (__dict__)
    pub attributes: Option<RefCell<HashMap<String, crate::vm::bytecode::Value>>>,
}

impl Object {
    /// 새 객체 생성 (속성 없이)
    pub fn new(type_id: u16, data: ObjectData) -> Self {
        Self {
            type_id,
            data,
            attributes: None,
        }
    }

    /// 속성을 가질 수 있는 객체 생성
    pub fn new_with_attrs(type_id: u16, data: ObjectData) -> Self {
        Self {
            type_id,
            data,
            attributes: Some(RefCell::new(HashMap::new())),
        }
    }

    pub fn get_attr(&self, name: &str) -> Option<crate::vm::bytecode::Value> {
        self.attributes
            .as_ref()
            .and_then(|attrs| attrs.borrow().get(name).cloned())
    }

    pub fn set_attr(&mut self, name: String, value: crate::vm::bytecode::Value) {
        if self.attributes.is_none() {
            self.attributes = Some(RefCell::new(HashMap::new()));
        }
        if let Some(ref attrs) = self.attributes {
            attrs.borrow_mut().insert(name, value);
        }
    }
}

#[derive(Debug, Clone)]
pub enum ObjectData {
    String(String),

    /// List (mutable)
    List {
        items: RefCell<Vec<crate::vm::bytecode::Value>>,
    },

    /// Dict (mutable)
    Dict {
        map: RefCell<HashMap<DictKey, crate::vm::bytecode::Value>>,
    },

    /// 사용자 정의 클래스
    UserClass {
        class_id: u16,
        methods: HashMap<String, u16>, // method_name -> func_id
    },

    /// 사용자 정의 객체 인스턴스
    UserInstance {
        class_id: u16,
    },

    /// Builtin 클래스
    BuiltinClass {
        class_type: BuiltinClassType,
    },

    /// Builtin 인스턴스
    BuiltinInstance {
        class_type: BuiltinClassType,
        data: BuiltinInstanceData,
    },

    /// User-defined function/lambda (closure)
    UserFunction {
        func_id: u16,
        captures: Vec<crate::vm::bytecode::Value>,
    },
}

/// Dict key wrapper (hashable types only)
/// TODO: __hash__ 메서드 구현 시 가능하도록 수정 필요
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum DictKey {
    Int(i64),
    String(String),
    Bool(bool),
}

#[derive(Debug, Clone)]
pub enum BuiltinInstanceData {
    /// Range iterator 상태
    Range {
        current: RefCell<i64>,
        stop: i64,
        step: i64,
    },

    /// List iterator 상태
    ListIterator {
        items: Rc<RefCell<Vec<crate::vm::bytecode::Value>>>,
        current: RefCell<usize>,
    },

    /// Dict iterator 상태 (keys iterator)
    DictIterator {
        keys: Vec<DictKey>,
        current: RefCell<usize>,
    },

    /// Map iterator 상태
    MapIterator {
        func: Box<crate::vm::bytecode::Value>,
        source_iter: Box<crate::vm::bytecode::Value>,
    },

    /// Filter iterator 상태
    FilterIterator {
        func: Box<crate::vm::bytecode::Value>,
        source_iter: Box<crate::vm::bytecode::Value>,
        /// 미리 찾아둔 다음 값 (peek buffer)
        peeked: std::cell::RefCell<Option<crate::vm::bytecode::Value>>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_object_creation() {
        let obj = Object::new(0, ObjectData::String("hello".to_string()));
        assert_eq!(obj.type_id, 0);
        assert!(matches!(obj.data, ObjectData::String(_)));
        assert!(obj.attributes.is_none());
    }

    #[test]
    fn test_object_with_attributes() {
        use crate::builtins::TYPE_STR;

        let mut obj = Object::new_with_attrs(1, ObjectData::UserInstance { class_id: 0 });
        assert!(obj.attributes.is_some());

        // 속성 설정
        let string_obj = crate::vm::bytecode::Value::Object(std::rc::Rc::new(Object::new(
            TYPE_STR,
            ObjectData::String("test".to_string()),
        )));
        obj.set_attr("name".to_string(), string_obj);

        // 속성 가져오기
        let value = obj.get_attr("name");
        assert!(value.is_some());
    }

    #[test]
    fn test_lazy_attribute_allocation() {
        let mut obj = Object::new(2, ObjectData::String("test".to_string()));
        assert!(obj.attributes.is_none());

        // 첫 속성 설정 시 할당됨
        obj.set_attr("key".to_string(), crate::vm::bytecode::Value::Int(42));
        assert!(obj.attributes.is_some());
    }
}
