//! Builtin 함수들의 유닛 테스트
//!
//! 각 builtin 함수의 성공/실패 케이스를 테스트합니다.

use super::*;
use crate::runtime_io::{ReadResult, RuntimeIo};

// ========== int() 테스트 ==========

#[test]
fn test_int_from_int() {
    let result = int::call(vec![Value::Int(42)]).unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_int_from_bool_true() {
    let result = int::call(vec![Value::Bool(true)]).unwrap();
    assert_eq!(result, Value::Int(1));
}

#[test]
fn test_int_from_bool_false() {
    let result = int::call(vec![Value::Bool(false)]).unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_int_from_string_valid() {
    let args = vec![make_string("42".into())];
    let result = int::call(args).unwrap();
    assert_eq!(result, Value::Int(42));
}

#[test]
fn test_int_from_string_negative() {
    let args = vec![make_string("-100".into())];
    let result = int::call(args).unwrap();
    assert_eq!(result, Value::Int(-100));
}

#[test]
fn test_int_from_string_with_whitespace() {
    let args = vec![make_string("  123  ".into())];
    let result = int::call(args).unwrap();
    assert_eq!(result, Value::Int(123));
}

#[test]
fn test_int_from_string_invalid() {
    let args = vec![make_string("abc".into())];
    let err = int::call(args).unwrap_err();
    assert!(err.message.contains("invalid literal"));
    assert!(err.message.contains("abc"));
}

#[test]
fn test_int_from_string_empty() {
    let args = vec![make_string("".into())];
    let err = int::call(args).unwrap_err();
    assert!(err.message.contains("invalid literal"));
}

#[test]
fn test_int_from_none() {
    let err = int::call(vec![Value::None]).unwrap_err();
    assert!(err.message.contains("int() argument must be"));
    assert!(err.message.contains("NoneType"));
}

#[test]
fn test_int_arity_zero() {
    let err = int::call(vec![]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 0
        }
    ));
}

#[test]
fn test_int_arity_two() {
    let err = int::call(vec![Value::Int(1), Value::Int(2)]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 2
        }
    ));
}

// ========== bool() 테스트 ==========

#[test]
fn test_bool_from_bool() {
    assert_eq!(
        bool_builtin::call(vec![Value::Bool(true)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        bool_builtin::call(vec![Value::Bool(false)]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn test_bool_from_int_zero() {
    assert_eq!(
        bool_builtin::call(vec![Value::Int(0)]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn test_bool_from_int_nonzero() {
    assert_eq!(
        bool_builtin::call(vec![Value::Int(1)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        bool_builtin::call(vec![Value::Int(-5)]).unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        bool_builtin::call(vec![Value::Int(100)]).unwrap(),
        Value::Bool(true)
    );
}

#[test]
fn test_bool_from_none() {
    assert_eq!(
        bool_builtin::call(vec![Value::None]).unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn test_bool_from_string_empty() {
    let args = vec![make_string("".into())];
    assert_eq!(bool_builtin::call(args).unwrap(), Value::Bool(false));
}

#[test]
fn test_bool_from_string_nonempty() {
    let args = vec![make_string("hello".into())];
    assert_eq!(bool_builtin::call(args).unwrap(), Value::Bool(true));
}

#[test]
fn test_bool_arity_zero() {
    let err = bool_builtin::call(vec![]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 0
        }
    ));
}

#[test]
fn test_bool_arity_two() {
    let err = bool_builtin::call(vec![Value::Bool(true), Value::Bool(false)]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 2
        }
    ));
}

// ========== str() 테스트 ==========

#[test]
fn test_str_from_int() {
    let result = str_builtin::call(vec![Value::Int(42)]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "42");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_from_bool() {
    let result = str_builtin::call(vec![Value::Bool(true)]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "True");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_from_none() {
    let result = str_builtin::call(vec![Value::None]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "None");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_arity_zero() {
    let err = str_builtin::call(vec![]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 0
        }
    ));
}

// ========== len() 테스트 ==========

#[test]
fn test_len_string_empty() {
    let args = vec![make_string("".into())];
    let result = len::call(args).unwrap();
    assert_eq!(result, Value::Int(0));
}

#[test]
fn test_len_string_ascii() {
    let args = vec![make_string("hello".into())];
    let result = len::call(args).unwrap();
    assert_eq!(result, Value::Int(5));
}

#[test]
fn test_len_string_unicode() {
    let args = vec![make_string("안녕".into())];
    let result = len::call(args).unwrap();
    assert_eq!(result, Value::Int(2));
}

#[test]
fn test_len_int_error() {
    let err = len::call(vec![Value::Int(42)]).unwrap_err();
    assert!(err.message.contains("has no len()"));
}

#[test]
fn test_len_none_error() {
    let err = len::call(vec![Value::None]).unwrap_err();
    assert!(err.message.contains("has no len()"));
}

#[test]
fn test_len_arity_zero() {
    let err = len::call(vec![]).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 0
        }
    ));
}

#[test]
fn test_len_arity_two() {
    let args = vec![make_string("a".into()), make_string("b".into())];
    let err = len::call(args).unwrap_err();
    assert!(matches!(
        err.kind,
        VmErrorKind::ArityError {
            expected: 1,
            got: 2
        }
    ));
}

// ========== range() 테스트 ==========

#[test]
fn test_range_one_arg() {
    let result = range::create_range(vec![Value::Int(5)]).unwrap();
    // range 객체가 생성되었는지 확인
    if let Value::Object(obj) = result {
        assert!(matches!(
            obj.data,
            crate::vm::value::ObjectData::BuiltinInstance { .. }
        ));
    } else {
        panic!("Expected range object");
    }
}

#[test]
fn test_range_two_args() {
    let result = range::create_range(vec![Value::Int(2), Value::Int(5)]).unwrap();
    if let Value::Object(obj) = result {
        assert!(matches!(
            obj.data,
            crate::vm::value::ObjectData::BuiltinInstance { .. }
        ));
    } else {
        panic!("Expected range object");
    }
}

#[test]
fn test_range_three_args() {
    let result = range::create_range(vec![Value::Int(0), Value::Int(10), Value::Int(2)]).unwrap();
    if let Value::Object(obj) = result {
        assert!(matches!(
            obj.data,
            crate::vm::value::ObjectData::BuiltinInstance { .. }
        ));
    } else {
        panic!("Expected range object");
    }
}

#[test]
fn test_range_zero_step() {
    let err = range::create_range(vec![Value::Int(0), Value::Int(10), Value::Int(0)]).unwrap_err();
    assert!(err.message.contains("must not be zero"));
}

#[test]
fn test_range_non_int_arg() {
    let err = range::create_range(vec![Value::Bool(true)]).unwrap_err();
    assert!(err.message.contains("must be int"));
}

#[test]
fn test_range_arity_zero() {
    let err = range::create_range(vec![]).unwrap_err();
    assert!(err.message.contains("1 to 3 arguments"));
}

#[test]
fn test_range_arity_four() {
    let err = range::create_range(vec![
        Value::Int(0),
        Value::Int(10),
        Value::Int(2),
        Value::Int(5),
    ])
    .unwrap_err();
    assert!(err.message.contains("1 to 3 arguments"));
}

// ========== print() 테스트 ==========

struct MockIo {
    output: std::cell::RefCell<Vec<String>>,
}

impl MockIo {
    fn new() -> Self {
        Self {
            output: std::cell::RefCell::new(Vec::new()),
        }
    }

    fn get_output(&self) -> Vec<String> {
        self.output.borrow().clone()
    }
}

impl RuntimeIo for MockIo {
    fn write_line(&mut self, s: &str) {
        self.output.borrow_mut().push(s.to_string());
    }

    fn write(&mut self, s: &str) {
        // write는 무시 (테스트용)
        let _ = s;
    }

    fn read_line(&mut self) -> ReadResult {
        ReadResult::Error("not implemented".to_string())
    }

    fn read_line_with_prompt(&mut self, _prompt: Option<&str>) -> ReadResult {
        ReadResult::Error("not implemented".to_string())
    }
}

#[test]
fn test_print_no_args() {
    let mut io = MockIo::new();
    let result = print::call(vec![], &mut io).unwrap();
    assert_eq!(result, Value::None);
    assert_eq!(io.get_output(), vec![""]);
}

#[test]
fn test_print_int() {
    let mut io = MockIo::new();
    let result = print::call(vec![Value::Int(42)], &mut io).unwrap();
    assert_eq!(result, Value::None);
    assert_eq!(io.get_output(), vec!["42"]);
}

#[test]
fn test_print_bool() {
    let mut io = MockIo::new();
    let result = print::call(vec![Value::Bool(true)], &mut io).unwrap();
    assert_eq!(result, Value::None);
    assert_eq!(io.get_output(), vec!["True"]);
}

#[test]
fn test_print_multiple_args() {
    let mut io = MockIo::new();
    let result = print::call(vec![Value::Int(1), Value::Bool(true), Value::None], &mut io).unwrap();
    assert_eq!(result, Value::None);
    assert_eq!(io.get_output(), vec!["1 True None"]);
}

// ========== String 메서드 테스트 ==========

#[test]
fn test_str_upper() {
    let receiver = make_string("hello".into());
    let result = str_builtin::str_upper(&receiver, vec![]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "HELLO");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_lower() {
    let receiver = make_string("WORLD".into());
    let result = str_builtin::str_lower(&receiver, vec![]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "world");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_strip() {
    let receiver = make_string("  trim me  ".into());
    let result = str_builtin::str_strip(&receiver, vec![]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "trim me");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_replace() {
    let receiver = make_string("hello world".into());
    let old = make_string("world".into());
    let new = make_string("rust".into());
    let result = str_builtin::str_replace(&receiver, vec![old, new]).unwrap();
    if let Value::Object(obj) = result {
        if let crate::vm::value::ObjectData::String(s) = &obj.data {
            assert_eq!(s, "hello rust");
        } else {
            panic!("Expected String object");
        }
    } else {
        panic!("Expected Object");
    }
}

#[test]
fn test_str_startswith_true() {
    let receiver = make_string("hello".into());
    let prefix = make_string("hel".into());
    let result = str_builtin::str_starts_with(&receiver, vec![prefix]).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_str_startswith_false() {
    let receiver = make_string("hello".into());
    let prefix = make_string("world".into());
    let result = str_builtin::str_starts_with(&receiver, vec![prefix]).unwrap();
    assert_eq!(result, Value::Bool(false));
}

#[test]
fn test_str_endswith_true() {
    let receiver = make_string("hello".into());
    let suffix = make_string("lo".into());
    let result = str_builtin::str_ends_with(&receiver, vec![suffix]).unwrap();
    assert_eq!(result, Value::Bool(true));
}

#[test]
fn test_str_find_found() {
    let receiver = make_string("hello world".into());
    let substr = make_string("world".into());
    let result = str_builtin::str_find(&receiver, vec![substr]).unwrap();
    assert_eq!(result, Value::Int(6));
}

#[test]
fn test_str_find_not_found() {
    let receiver = make_string("hello".into());
    let substr = make_string("xyz".into());
    let result = str_builtin::str_find(&receiver, vec![substr]).unwrap();
    assert_eq!(result, Value::Int(-1));
}

#[test]
fn test_str_count() {
    let receiver = make_string("hello hello hello".into());
    let substr = make_string("hello".into());
    let result = str_builtin::str_count(&receiver, vec![substr]).unwrap();
    assert_eq!(result, Value::Int(3));
}
