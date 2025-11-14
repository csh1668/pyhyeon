// JIT 바이트코드 컴파일러
//
// Pyhyeon 바이트코드를 Cranelift IR로 변환합니다.

use cranelift::prelude::*;
use cranelift_codegen::ir::InstBuilder;
use cranelift_codegen::isa::CallConv;
use cranelift_jit::JITModule;
use cranelift_module::{FuncId, Linkage, Module as ClifModule};
use std::collections::HashMap;

use crate::vm::bytecode::{FunctionCode, Instruction as I};
use crate::vm::machine::{err, VmErrorKind, VmResult};

/// 바이트코드를 Cranelift IR로 컴파일
pub fn compile_function(
    builder: &mut FunctionBuilder,
    func_code: &FunctionCode,
    module: &mut JITModule,
) -> VmResult<()> {
    // 엔트리 블록 생성
    let entry_block = builder.create_block();
    builder.switch_to_block(entry_block);
    builder.append_block_params_for_function_params(entry_block);

    // VM 포인터 파라미터
    let vm_ptr = builder.block_params(entry_block)[0];

    // 점프 타겟 추적 (바이트코드 인덱스 -> Cranelift 블록)
    let mut jump_targets: HashMap<usize, Block> = HashMap::new();

    // 1단계: 점프 타겟이 되는 위치의 블록 미리 생성
    for (ip, ins) in func_code.code.iter().enumerate() {
        match ins {
            I::Jump(offset) | I::JumpIfFalse(offset) | I::JumpIfTrue(offset) => {
                let target = (ip as i32 + 1 + offset) as usize;
                if !jump_targets.contains_key(&target) {
                    jump_targets.insert(target, builder.create_block());
                }
            }
            _ => {}
        }
    }

    // 2단계: 바이트코드를 IR로 변환
    let mut ctx = CompilerContext::new(
        vm_ptr,
        builder.func.dfg.value_type(vm_ptr),
        func_code.num_locals as usize,
    );

    // 함수 파라미터를 로컬 변수로 초기화 (런타임에서 이미 프레임에 설정됨)
    // 현재는 런타임 헬퍼를 통해 접근하므로, 최적화된 버전에서는
    // 파라미터를 직접 전달받을 수 있도록 개선 가능

    let mut last_was_terminator = false;

    for (ip, ins) in func_code.code.iter().enumerate() {
        // 이 위치가 점프 타겟이면 블록 전환
        if let Some(&target_block) = jump_targets.get(&ip) {
            // 현재 블록이 아직 종료되지 않았으면 fallthrough jump 추가
            if !last_was_terminator {
                builder.ins().jump(target_block, &[]);
            }
            builder.switch_to_block(target_block);
            last_was_terminator = false;
        }

        // Terminator 이후의 unreachable 명령어는 건너뛰기
        // (점프 타겟이 아닌 경우에만)
        if last_was_terminator && !jump_targets.contains_key(&ip) {
            continue;
        }

        // 명령어 컴파일
        compile_instruction(builder, &mut ctx, ins, ip, &jump_targets, module)?;

        // 이 명령어가 terminator인지 체크 (Jump, JumpIfFalse, JumpIfTrue, Return)
        last_was_terminator = matches!(
            ins,
            I::Jump(_) | I::JumpIfFalse(_) | I::JumpIfTrue(_) | I::Return
        );
    }

    // 함수 종료 (성공 반환) - 마지막이 terminator가 아니면 추가
    if !last_was_terminator {
        let zero = builder.ins().iconst(types::I64, 0);
        builder.ins().return_(&[zero]);
    }

    builder.seal_all_blocks();
    // Note: finalize() consumes the builder, so we don't call it here
    // The FunctionBuilder will be dropped automatically

    Ok(())
}

/// 컴파일러 컨텍스트
struct CompilerContext {
    /// VM 포인터 값
    vm_ptr: Value,

    /// VM 포인터 타입
    ptr_type: Type,

    /// 가상 스택 (SSA 값 추적)
    /// 바이트코드 스택의 각 슬롯을 Cranelift Value로 매핑
    value_stack: Vec<Value>,

    /// 로컬 변수 (SSA 값)
    /// 인덱스 -> Cranelift Value 매핑
    locals: Vec<Option<Value>>,
}

impl CompilerContext {
    fn new(vm_ptr: Value, ptr_type: Type, num_locals: usize) -> Self {
        Self {
            vm_ptr,
            ptr_type,
            value_stack: Vec::new(),
            locals: vec![None; num_locals],
        }
    }

    /// 스택에 값 푸시
    fn push(&mut self, val: Value) {
        self.value_stack.push(val);
    }

    /// 스택에서 값 팝
    fn pop(&mut self) -> Option<Value> {
        self.value_stack.pop()
    }

    /// 로컬 변수 설정
    fn set_local(&mut self, index: usize, val: Value) {
        if index < self.locals.len() {
            self.locals[index] = Some(val);
        }
    }

    /// 로컬 변수 가져오기
    fn get_local(&self, index: usize) -> Option<Value> {
        self.locals.get(index).and_then(|v| *v)
    }
}

/// 단일 명령어를 IR로 컴파일
fn compile_instruction(
    builder: &mut FunctionBuilder,
    ctx: &mut CompilerContext,
    ins: &I,
    ip: usize,
    jump_targets: &HashMap<usize, Block>,
    module: &mut JITModule,
) -> VmResult<()> {
    match ins {
        // ===== 상수 =====
        I::ConstI64(i) => {
            // 최적화: 상수를 SSA 값으로 직접 푸시
            let value = builder.ins().iconst(types::I64, *i);
            ctx.push(value);
        }

        I::True => {
            let one = builder.ins().iconst(types::I64, 1);
            ctx.push(one);
        }

        I::False => {
            let zero = builder.ins().iconst(types::I64, 0);
            ctx.push(zero);
        }

        // ===== 로컬 변수 =====
        I::LoadLocal(ix) => {
            let index = *ix as usize;
            // 최적화: 로컬 변수가 SSA 값으로 추적되면 직접 사용
            if let Some(val) = ctx.get_local(index) {
                ctx.push(val);
            } else {
                // Fallback: VM 프레임에서 로드하여 SSA 값으로 변환
                // pyhyeon_load_local_to_value(vm_ptr, index) -> i64 값 반환
                let index_val = builder.ins().iconst(types::I16, index as i64);
                let loaded_val = call_runtime_helper(
                    builder,
                    ctx,
                    "pyhyeon_load_local_value",
                    &[ctx.vm_ptr, index_val],
                    module,
                )?;
                ctx.push(loaded_val);
                // 추후 재사용을 위해 로컬 변수에도 저장
                ctx.set_local(index, loaded_val);
            }
        }

        I::StoreLocal(ix) => {
            let index = *ix as usize;
            // 최적화: 스택에서 SSA 값을 팝하여 로컬 변수에 저장
            if let Some(val) = ctx.pop() {
                ctx.set_local(index, val);
            } else {
                // 에러: SSA 스택이 비어있음 (JIT 컴파일 실패)
                return Err(err(
                    VmErrorKind::TypeError("jit"),
                    "StoreLocal: SSA stack underflow".into(),
                ));
            }
        }

        // ===== 산술 연산 (인라인 최적화) =====
        I::Add => {
            // 최적화: 스택에서 두 값을 팝하여 직접 덧셈
            if let (Some(b), Some(a)) = (ctx.pop(), ctx.pop()) {
                let result = builder.ins().iadd(a, b);
                ctx.push(result);
            } else {
                // Fallback: 런타임 헬퍼 호출
                call_runtime_helper(builder, ctx, "pyhyeon_add_int", &[ctx.vm_ptr], module)?;
            }
        }

        I::Sub => {
            if let (Some(b), Some(a)) = (ctx.pop(), ctx.pop()) {
                let result = builder.ins().isub(a, b);
                ctx.push(result);
            } else {
                call_runtime_helper(builder, ctx, "pyhyeon_sub_int", &[ctx.vm_ptr], module)?;
            }
        }

        I::Mul => {
            if let (Some(b), Some(a)) = (ctx.pop(), ctx.pop()) {
                let result = builder.ins().imul(a, b);
                ctx.push(result);
            } else {
                call_runtime_helper(builder, ctx, "pyhyeon_mul_int", &[ctx.vm_ptr], module)?;
            }
        }

        // ===== 비교 연산 (인라인 최적화) =====
        I::Eq => {
            if let (Some(b), Some(a)) = (ctx.pop(), ctx.pop()) {
                let cmp = builder.ins().icmp(IntCC::Equal, a, b);
                // bool을 i64로 확장 (0 또는 1)
                // select(cmp, 1, 0)
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                ctx.push(result);
            } else {
                call_runtime_helper(builder, ctx, "pyhyeon_eq_int", &[ctx.vm_ptr], module)?;
            }
        }

        I::Lt => {
            if let (Some(b), Some(a)) = (ctx.pop(), ctx.pop()) {
                let cmp = builder.ins().icmp(IntCC::SignedLessThan, a, b);
                // bool을 i64로 확장
                let one = builder.ins().iconst(types::I64, 1);
                let zero = builder.ins().iconst(types::I64, 0);
                let result = builder.ins().select(cmp, one, zero);
                ctx.push(result);
            } else {
                call_runtime_helper(builder, ctx, "pyhyeon_lt_int", &[ctx.vm_ptr], module)?;
            }
        }

        // ===== 제어 흐름 =====
        I::Jump(offset) => {
            let target = (ip as i32 + 1 + offset) as usize;
            let target_block = *jump_targets.get(&target).ok_or_else(|| {
                err(
                    VmErrorKind::TypeError("jit"),
                    format!("Jump target {} not found", target),
                )
            })?;
            builder.ins().jump(target_block, &[]);
        }

        I::JumpIfFalse(offset) => {
            let target = (ip as i32 + 1 + offset) as usize;
            let target_block = *jump_targets.get(&target).ok_or_else(|| {
                err(
                    VmErrorKind::TypeError("jit"),
                    format!("Jump target {} not found", target),
                )
            })?;

            // 스택에서 조건 팝 (pyhyeon_pop_bool)
            let cond_ptr = builder.create_sized_stack_slot(StackSlotData::new(
                StackSlotKind::ExplicitSlot,
                8,
                0,
            ));
            let cond_addr = builder.ins().stack_addr(ctx.ptr_type, cond_ptr, 0);

            let pop_result = call_runtime_helper(
                builder,
                ctx,
                "pyhyeon_pop_bool",
                &[ctx.vm_ptr, cond_addr],
                module,
            )?;

            // 에러 체크 (result < 0)
            let zero = builder.ins().iconst(types::I64, 0);
            let is_err = builder.ins().icmp(IntCC::SignedLessThan, pop_result, zero);

            let err_block = builder.create_block();
            let ok_block = builder.create_block();

            builder.ins().brif(is_err, err_block, &[], ok_block, &[]);

            // 에러 블록: 즉시 반환
            builder.switch_to_block(err_block);
            builder.ins().return_(&[pop_result]);

            // OK 블록: 조건 체크
            builder.switch_to_block(ok_block);
            let cond_val = builder.ins().load(types::I64, MemFlags::new(), cond_addr, 0);
            let is_false = builder.ins().icmp_imm(IntCC::Equal, cond_val, 0);

            // 다음 명령어 블록
            let next_ip = ip + 1;
            let next_block = if let Some(&b) = jump_targets.get(&next_ip) {
                b
            } else {
                builder.create_block()
            };

            builder.ins().brif(is_false, target_block, &[], next_block, &[]);

            // 다음 블록으로 전환 (jump_targets에 없으면 여기서 계속)
            if !jump_targets.contains_key(&next_ip) {
                builder.switch_to_block(next_block);
            }
        }

        I::Return => {
            // 최적화: SSA 스택에 값이 있으면 VM 스택에 푸시
            if let Some(return_val) = ctx.pop() {
                // pyhyeon_push_int(vm_ptr, return_val)
                call_runtime_helper(
                    builder,
                    ctx,
                    "pyhyeon_push_int",
                    &[ctx.vm_ptr, return_val],
                    module,
                )?;
            }
            // 성공 반환 (0 = 성공)
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
        }

        // ===== 아직 지원하지 않는 명령어 =====
        _ => {
            // Fallback: 인터프리터로 되돌아감
            // 지원하지 않는 명령어는 JIT를 중단하고 인터프리터로 전환
            return Err(err(
                VmErrorKind::TypeError("jit"),
                format!("Unsupported instruction for JIT: {:?}", ins),
            ));
        }
    }

    Ok(())
}

/// 런타임 헬퍼 함수 호출
///
/// 외부 C 함수를 호출하고 에러 체크를 수행합니다.
fn call_runtime_helper(
    builder: &mut FunctionBuilder,
    ctx: &CompilerContext,
    func_name: &str,
    args: &[Value],
    module: &mut JITModule,
) -> VmResult<Value> {
    // 함수 시그니처 생성
    let sig = create_helper_signature(func_name, module);

    // 외부 함수 선언 (이미 등록된 심볼)
    let func_id = module
        .declare_function(func_name, Linkage::Import, &sig)
        .map_err(|e| {
            err(
                VmErrorKind::TypeError("jit"),
                format!("Failed to declare function {}: {}", func_name, e),
            )
        })?;

    // 함수 참조 가져오기
    let local_func = module.declare_func_in_func(func_id, builder.func);

    // 함수 호출
    let call = builder.ins().call(local_func, args);
    let result = builder.inst_results(call)[0];

    Ok(result)
}

/// 런타임 헬퍼 함수의 시그니처 생성
fn create_helper_signature(func_name: &str, module: &JITModule) -> Signature {
    // 플랫폼 기본 calling convention 사용 (Windows: fastcall, Unix: systemv)
    let call_conv = module.target_config().default_call_conv;
    let mut sig = Signature::new(call_conv);
    let ptr_type = module.target_config().pointer_type();

    match func_name {
        "pyhyeon_push_int" => {
            sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
            sig.params.push(AbiParam::new(types::I64)); // value
            sig.returns.push(AbiParam::new(types::I64)); // result
        }
        "pyhyeon_push_bool" => {
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I64));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "pyhyeon_pop_int" => {
            sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
            sig.params.push(AbiParam::new(ptr_type)); // out_value ptr
            sig.returns.push(AbiParam::new(types::I64));
        }
        "pyhyeon_pop_bool" => {
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(ptr_type));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "pyhyeon_load_local" => {
            sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
            sig.params.push(AbiParam::new(types::I16)); // index
            sig.returns.push(AbiParam::new(types::I64));
        }
        "pyhyeon_load_local_value" => {
            sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
            sig.params.push(AbiParam::new(types::I16)); // index
            sig.returns.push(AbiParam::new(types::I64)); // 직접 값 반환
        }
        "pyhyeon_store_local" => {
            sig.params.push(AbiParam::new(ptr_type));
            sig.params.push(AbiParam::new(types::I16));
            sig.returns.push(AbiParam::new(types::I64));
        }
        "pyhyeon_add_int" | "pyhyeon_sub_int" | "pyhyeon_mul_int" | "pyhyeon_eq_int"
        | "pyhyeon_lt_int" => {
            sig.params.push(AbiParam::new(ptr_type)); // vm_ptr
            sig.returns.push(AbiParam::new(types::I64));
        }
        _ => panic!("Unknown runtime helper: {}", func_name),
    }

    sig
}
