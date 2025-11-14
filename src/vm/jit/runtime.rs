// JIT 런타임 인터페이스
//
// JIT 컴파일된 네이티브 코드가 VM과 상호작용하기 위한 인터페이스입니다.

use crate::vm::bytecode::Value;
use crate::vm::machine::{err, Vm, VmErrorKind, VmResult};

/// 네이티브 함수 타입
///
/// JIT 컴파일된 함수의 시그니처:
/// - 인자: VM 포인터 (raw pointer)
/// - 반환값: 성공(0) 또는 에러 코드
pub type NativeFunction = unsafe extern "C" fn(*mut Vm) -> i64;

/// 런타임 헬퍼: 스택에 값 푸시
///
/// # Safety
/// - vm_ptr은 유효한 Vm 포인터여야 합니다.
/// - 호출자는 스택 오버플로우를 체크해야 합니다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_push_int(vm_ptr: *mut Vm, value: i64) -> i64 {
    let vm = &mut *vm_ptr;
    match vm.push(Value::Int(value)) {
        Ok(()) => 0,
        Err(_) => -1, // 스택 오버플로우
    }
}

/// 런타임 헬퍼: 스택에서 정수 팝
///
/// # Safety
/// - vm_ptr은 유효한 Vm 포인터여야 합니다.
/// - out_value는 유효한 i64 포인터여야 합니다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_pop_int(vm_ptr: *mut Vm, out_value: *mut i64) -> i64 {
    let vm = &mut *vm_ptr;
    match vm.pop() {
        Ok(Value::Int(i)) => {
            *out_value = i;
            0
        }
        Ok(_) => -2, // 타입 에러
        Err(_) => -1, // 스택 언더플로우
    }
}

/// 런타임 헬퍼: 스택에 불린 푸시
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_push_bool(vm_ptr: *mut Vm, value: i64) -> i64 {
    let vm = &mut *vm_ptr;
    match vm.push(Value::Bool(value != 0)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 스택에서 불린 팝
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_pop_bool(vm_ptr: *mut Vm, out_value: *mut i64) -> i64 {
    let vm = &mut *vm_ptr;
    match vm.pop() {
        Ok(Value::Bool(b)) => {
            *out_value = if b { 1 } else { 0 };
            0
        }
        Ok(_) => -2,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 로컬 변수 로드 (VM 스택에 푸시)
///
/// # Safety
/// - vm_ptr은 유효한 Vm 포인터여야 합니다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_load_local(vm_ptr: *mut Vm, index: u16) -> i64 {
    unsafe {
        let vm = &mut *vm_ptr;

        // get_local은 private이므로 frames에 직접 접근
        if let Some(frame) = vm.frames.last() {
            if let Some(value) = frame.locals.get(index as usize) {
                match vm.push(value.clone()) {
                    Ok(()) => return 0,
                    Err(_) => return -1,
                }
            }
        }
        -3 // 프레임 없음 또는 잘못된 인덱스
    }
}

/// 런타임 헬퍼: 로컬 변수 값 직접 로드 (SSA 최적화용)
///
/// VM 스택에 푸시하지 않고 i64 값을 직접 반환합니다.
/// 정수가 아닌 경우 0을 반환합니다 (fallback).
///
/// # Safety
/// - vm_ptr은 유효한 Vm 포인터여야 합니다.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_load_local_value(vm_ptr: *mut Vm, index: u16) -> i64 {
    unsafe {
        let vm = &mut *vm_ptr;

        // frames에 직접 접근하여 값 가져오기
        if let Some(frame) = vm.frames.last() {
            if let Some(value) = frame.locals.get(index as usize) {
                // 정수 값만 지원 (fast path)
                if let Value::Int(i) = value {
                    return *i;
                }
            }
        }
        // 에러 또는 정수가 아닌 경우: JIT 컴파일 포기를 위해 특수 값 반환
        // 하지만 안전을 위해 0 반환
        0
    }
}

/// 런타임 헬퍼: 로컬 변수 저장
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_store_local(vm_ptr: *mut Vm, index: u16) -> i64 {
    let vm = &mut *vm_ptr;

    let value = match vm.pop() {
        Ok(v) => v,
        Err(_) => return -1,
    };

    if let Some(frame) = vm.frames.last_mut() {
        if let Some(slot) = frame.locals.get_mut(index as usize) {
            *slot = value;
            return 0;
        }
    }
    -3
}

/// 런타임 헬퍼: 정수 덧셈
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_add_int(vm_ptr: *mut Vm) -> i64 {
    let vm = &mut *vm_ptr;

    let b = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    let a = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    match vm.push(Value::Int(a.wrapping_add(b))) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 정수 뺄셈
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_sub_int(vm_ptr: *mut Vm) -> i64 {
    let vm = &mut *vm_ptr;

    let b = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    let a = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    match vm.push(Value::Int(a.wrapping_sub(b))) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 정수 곱셈
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_mul_int(vm_ptr: *mut Vm) -> i64 {
    let vm = &mut *vm_ptr;

    let b = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    let a = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    match vm.push(Value::Int(a.wrapping_mul(b))) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 정수 비교 (==)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_eq_int(vm_ptr: *mut Vm) -> i64 {
    let vm = &mut *vm_ptr;

    let b = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    let a = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    match vm.push(Value::Bool(a == b)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

/// 런타임 헬퍼: 정수 비교 (<)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn pyhyeon_lt_int(vm_ptr: *mut Vm) -> i64 {
    let vm = &mut *vm_ptr;

    let b = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    let a = match vm.pop() {
        Ok(Value::Int(i)) => i,
        _ => return -1,
    };

    match vm.push(Value::Bool(a < b)) {
        Ok(()) => 0,
        Err(_) => -1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vm::machine::Vm;

    #[test]
    fn test_push_pop_int() {
        let mut vm = Vm::new();
        let vm_ptr = &mut vm as *mut Vm;

        unsafe {
            // Push 123
            assert_eq!(pyhyeon_push_int(vm_ptr, 123), 0);
            assert_eq!(vm.stack.len(), 1);

            // Pop 123
            let mut value = 0;
            assert_eq!(pyhyeon_pop_int(vm_ptr, &mut value), 0);
            assert_eq!(value, 123);
            assert_eq!(vm.stack.len(), 0);
        }
    }

    #[test]
    fn test_add_int() {
        let mut vm = Vm::new();
        let vm_ptr = &mut vm as *mut Vm;

        unsafe {
            // Push 10 and 20
            pyhyeon_push_int(vm_ptr, 10);
            pyhyeon_push_int(vm_ptr, 20);

            // Add
            assert_eq!(pyhyeon_add_int(vm_ptr), 0);

            // Pop result
            let mut result = 0;
            pyhyeon_pop_int(vm_ptr, &mut result);
            assert_eq!(result, 30);
        }
    }

    #[test]
    fn test_comparison() {
        let mut vm = Vm::new();
        let vm_ptr = &mut vm as *mut Vm;

        unsafe {
            // 5 < 10 = true
            pyhyeon_push_int(vm_ptr, 5);
            pyhyeon_push_int(vm_ptr, 10);
            assert_eq!(pyhyeon_lt_int(vm_ptr), 0);

            let mut result = 0;
            pyhyeon_pop_bool(vm_ptr, &mut result);
            assert_eq!(result, 1); // true

            // 10 == 10 = true
            pyhyeon_push_int(vm_ptr, 10);
            pyhyeon_push_int(vm_ptr, 10);
            assert_eq!(pyhyeon_eq_int(vm_ptr), 0);

            pyhyeon_pop_bool(vm_ptr, &mut result);
            assert_eq!(result, 1); // true
        }
    }
}
