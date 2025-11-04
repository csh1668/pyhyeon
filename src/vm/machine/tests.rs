//! VM 테스트 모듈

use super::*;
use crate::vm::bytecode::{FunctionCode, Instruction as I, Module, Value};

fn make_test_module() -> Module {
    // Module::new()를 사용하면 타입 테이블이 자동으로 초기화됨
    Module::new()
}

// ========== 스택 연산 테스트 ==========

#[test]
fn test_stack_push_pop() {
    let mut vm = Vm::new();
    assert!(vm.push(Value::Int(42)).is_ok());
    assert_eq!(vm.pop().unwrap(), Value::Int(42));
}

#[test]
fn test_stack_underflow() {
    let mut vm = Vm::new();
    let result = vm.pop();
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.kind, VmErrorKind::StackUnderflow));
    }
}

#[test]
fn test_stack_overflow() {
    let mut vm = Vm::new();
    vm.max_stack = 2;
    assert!(vm.push(Value::Int(1)).is_ok());
    assert!(vm.push(Value::Int(2)).is_ok());
    let result = vm.push(Value::Int(3));
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.kind, VmErrorKind::StackOverflow));
    }
}

// ========== 명령어별 단위 테스트 ==========

#[test]
fn test_const_instructions() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42)],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    // Should return 42
    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_arithmetic_add() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(10), I::ConstI64(32), I::Add],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_arithmetic_sub() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(50), I::ConstI64(8), I::Sub],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_arithmetic_mul() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(6), I::ConstI64(7), I::Mul],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_arithmetic_div() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(84), I::ConstI64(2), I::Div],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_arithmetic_mod() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42), I::ConstI64(10), I::Mod],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(2)));
}

#[test]
fn test_arithmetic_neg() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42), I::Neg],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(-42)));
}

#[test]
fn test_comparison_eq() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42), I::ConstI64(42), I::Eq],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Bool(true)));
}

#[test]
fn test_comparison_lt() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(10), I::ConstI64(42), I::Lt],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Bool(true)));
}

#[test]
fn test_logical_not() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::True, I::Not],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Bool(false)));
}

#[test]
fn test_jump() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![
            I::Jump(2), // Skip next 2 instructions
            I::ConstI64(2),
            I::ConstI64(3),
            I::ConstI64(4),
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    // Should return 4 (skipped 2 and 3)
    assert_eq!(result, Some(Value::Int(4)));
}

#[test]
fn test_jump_if_false() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![
            I::False,
            I::JumpIfFalse(2), // Should jump
            I::ConstI64(1),
            I::ConstI64(2),
            I::ConstI64(3),
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    // Should return 3 (skipped 1 and 2)
    assert_eq!(result, Some(Value::Int(3)));
}

#[test]
fn test_local_variables() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 2,
        code: vec![
            I::ConstI64(42),
            I::StoreLocal(0),
            I::ConstI64(100),
            I::StoreLocal(1),
            I::LoadLocal(0),
            I::LoadLocal(1),
            I::Add,
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    // Should return 142
    assert_eq!(result, Some(Value::Int(142)));
}

// ========== 함수 호출 테스트 ==========

#[test]
fn test_function_call_no_args() {
    let mut module = make_test_module();

    // Function 0: main
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![
            I::Call(1, 0), // Call function 1 with 0 args
        ],
    });

    // Function 1: returns 42
    module.functions.push(FunctionCode {
        name_sym: 1,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42), I::Return],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_function_call_with_args() {
    let mut module = make_test_module();

    // Function 0: main, calls add(10, 32)
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![
            I::ConstI64(10),
            I::ConstI64(32),
            I::Call(1, 2), // Call function 1 with 2 args
        ],
    });

    // Function 1: add(a, b) -> a + b
    module.functions.push(FunctionCode {
        name_sym: 1,
        arity: 2,
        num_locals: 2,
        code: vec![
            I::LoadLocal(0), // a
            I::LoadLocal(1), // b
            I::Add,
            I::Return,
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    assert_eq!(result, Some(Value::Int(42)));
}

#[test]
fn test_recursive_function() {
    let mut module = make_test_module();

    // Function 0: main, calls factorial(5)
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(5), I::Call(1, 1)],
    });

    // Function 1: factorial(n)
    // if n == 0: return 1
    // else: return n * factorial(n-1)
    module.functions.push(FunctionCode {
        name_sym: 1,
        arity: 1,
        num_locals: 1,
        code: vec![
            I::LoadLocal(0), // n
            I::ConstI64(0),
            I::Eq,
            I::JumpIfFalse(2), // if n != 0, jump to else
            I::ConstI64(1),
            I::Return,
            // else:
            I::LoadLocal(0), // n
            I::LoadLocal(0), // n
            I::ConstI64(1),
            I::Sub,        // n - 1
            I::Call(1, 1), // factorial(n-1)
            I::Mul,        // n * factorial(n-1)
            I::Return,
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module).ok().flatten();

    // 5! = 120
    assert_eq!(result, Some(Value::Int(120)));
}

#[test]
fn test_zero_division_error() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![I::ConstI64(42), I::ConstI64(0), I::Div],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.kind, VmErrorKind::ZeroDivision));
    }
}

#[test]
fn test_type_error() {
    let mut module = make_test_module();
    module.functions.push(FunctionCode {
        name_sym: 0,
        arity: 0,
        num_locals: 0,
        code: vec![
            I::True,
            I::ConstI64(42),
            I::Add, // Can't add bool + int
        ],
    });

    let mut vm = Vm::new();
    let result = vm.run(&mut module);
    assert!(result.is_err());
    if let Err(e) = result {
        assert!(matches!(e.kind, VmErrorKind::TypeError(_)));
    }
}
