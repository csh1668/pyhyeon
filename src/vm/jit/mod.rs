// JIT 컴파일러 모듈
//
// Cranelift를 사용하여 Pyhyeon 바이트코드를 네이티브 코드로 컴파일합니다.
// Tiered compilation 전략을 사용합니다:
// 1. 처음에는 인터프리터로 실행
// 2. 함수 실행 횟수를 추적
// 3. Hot 함수(1000회 이상 실행)를 JIT 컴파일

use cranelift::prelude::*;
use cranelift_codegen::settings;
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{Linkage, Module as ClifModule};
use std::collections::HashMap;

use crate::vm::bytecode::{FunctionCode, Instruction as I};
use crate::vm::machine::{err, VmErrorKind, VmResult};

mod compiler;
mod runtime;

pub use runtime::NativeFunction;

/// JIT 컴파일 임계값 (함수가 이만큼 실행되면 JIT 컴파일)
pub const JIT_THRESHOLD: u32 = 1000;

/// JIT 컴파일러
///
/// Cranelift를 사용하여 바이트코드를 네이티브 코드로 컴파일합니다.
pub struct JitCompiler {
    /// Cranelift JIT 모듈
    module: JITModule,

    /// 함수 빌더 컨텍스트 (재사용)
    func_ctx: codegen::Context,

    /// 컴파일된 함수 캐시 (function_id -> native_function)
    compiled_functions: HashMap<usize, NativeFunction>,
}

impl JitCompiler {
    /// 새 JIT 컴파일러 생성
    pub fn new() -> VmResult<Self> {
        let mut flag_builder = settings::builder();

        // 최적화 레벨 설정 (baseline JIT이므로 빠른 컴파일 우선)
        flag_builder.set("opt_level", "speed").map_err(|e| {
            err(
                VmErrorKind::TypeError("jit"),
                format!("Failed to set opt_level: {}", e),
            )
        })?;

        // ISA 생성 (target architecture)
        let isa_builder = cranelift_codegen::isa::lookup(target_lexicon::HOST).map_err(|e| {
            err(
                VmErrorKind::TypeError("jit"),
                format!("Failed to lookup ISA: {}", e),
            )
        })?;

        let isa = isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| {
            err(
                VmErrorKind::TypeError("jit"),
                format!("Failed to create ISA: {}", e),
            )
        })?;

        // JIT 모듈 생성
        let mut builder = JITBuilder::with_isa(isa, cranelift_module::default_libcall_names());

        // 런타임 헬퍼 심볼 등록
        builder.symbol("pyhyeon_push_int", runtime::pyhyeon_push_int as *const u8);
        builder.symbol("pyhyeon_pop_int", runtime::pyhyeon_pop_int as *const u8);
        builder.symbol("pyhyeon_push_bool", runtime::pyhyeon_push_bool as *const u8);
        builder.symbol("pyhyeon_pop_bool", runtime::pyhyeon_pop_bool as *const u8);
        builder.symbol("pyhyeon_load_local", runtime::pyhyeon_load_local as *const u8);
        builder.symbol("pyhyeon_load_local_value", runtime::pyhyeon_load_local_value as *const u8);
        builder.symbol("pyhyeon_store_local", runtime::pyhyeon_store_local as *const u8);
        builder.symbol("pyhyeon_add_int", runtime::pyhyeon_add_int as *const u8);
        builder.symbol("pyhyeon_sub_int", runtime::pyhyeon_sub_int as *const u8);
        builder.symbol("pyhyeon_mul_int", runtime::pyhyeon_mul_int as *const u8);
        builder.symbol("pyhyeon_eq_int", runtime::pyhyeon_eq_int as *const u8);
        builder.symbol("pyhyeon_lt_int", runtime::pyhyeon_lt_int as *const u8);

        let module = JITModule::new(builder);

        Ok(Self {
            module,
            func_ctx: codegen::Context::new(),
            compiled_functions: HashMap::new(),
        })
    }

    /// 함수를 JIT 컴파일
    ///
    /// 바이트코드를 Cranelift IR로 변환한 후 네이티브 코드로 컴파일합니다.
    pub fn compile(&mut self, func_id: usize, func_code: &FunctionCode) -> VmResult<NativeFunction> {
        // 이미 컴파일된 함수는 캐시에서 반환
        if let Some(native_fn) = self.compiled_functions.get(&func_id) {
            return Ok(*native_fn);
        }

        // 함수 시그니처 생성
        self.func_ctx.func.signature = self.create_function_signature();

        // 함수 빌더 생성
        let mut builder_ctx = FunctionBuilderContext::new();
        let mut builder = FunctionBuilder::new(&mut self.func_ctx.func, &mut builder_ctx);

        // IR 생성 (바이트코드 -> Cranelift IR)
        compiler::compile_function(&mut builder, func_code, &mut self.module)?;

        // 함수 ID 선언
        let func_name = format!("pyhyeon_func_{}", func_id);
        let func_id_clif = self
            .module
            .declare_function(&func_name, Linkage::Local, &self.func_ctx.func.signature)
            .map_err(|e| {
                err(
                    VmErrorKind::TypeError("jit"),
                    format!("Failed to declare function: {}", e),
                )
            })?;

        // 함수 정의 (IR -> 네이티브 코드)
        // Note: define_function이 자동으로 verify를 수행합니다
        self.module
            .define_function(func_id_clif, &mut self.func_ctx)
            .map_err(|e| {
                // 디버그: 에러 발생 시 IR 출력
                #[cfg(debug_assertions)]
                {
                    eprintln!("[JIT] Failed to define function: {}", e);
                    eprintln!("[JIT] Function IR:");
                    eprintln!("{}", self.func_ctx.func.display());
                }
                err(
                    VmErrorKind::TypeError("jit"),
                    format!("Failed to define function: {}", e),
                )
            })?;

        // 함수 최종화
        self.module.finalize_definitions().map_err(|e| {
            err(
                VmErrorKind::TypeError("jit"),
                format!("Failed to finalize function: {}", e),
            )
        })?;

        // 네이티브 함수 포인터 가져오기
        let code_ptr = self.module.get_finalized_function(func_id_clif);
        let native_fn: NativeFunction = unsafe { std::mem::transmute(code_ptr) };

        // 캐시에 저장
        self.compiled_functions.insert(func_id, native_fn);

        Ok(native_fn)
    }

    /// Cranelift 함수 시그니처 생성
    ///
    /// 모든 Pyhyeon 함수는 동일한 시그니처를 가집니다:
    /// `i64 function(void* vm_ptr)`
    ///
    /// - 인자: VM 포인터 (스택, 프레임 등 접근)
    /// - 반환값: 성공(0) 또는 에러 코드
    fn create_function_signature(&self) -> Signature {
        let mut sig = self.module.make_signature();

        // 인자: VM 포인터
        let pointer_type = self.module.target_config().pointer_type();
        sig.params.push(AbiParam::new(pointer_type));

        // 반환값: i64 (0 = 성공, 음수 = 에러)
        sig.returns.push(AbiParam::new(types::I64));

        sig
    }
}

impl Default for JitCompiler {
    fn default() -> Self {
        Self::new().expect("Failed to create JIT compiler")
    }
}

/// Hot path 추적기
///
/// 함수별 실행 횟수를 추적하여 JIT 컴파일 여부를 결정합니다.
#[derive(Debug, Default)]
pub struct HotPathTracker {
    /// 함수별 실행 횟수
    pub counters: HashMap<usize, u32>,

    /// JIT 컴파일 임계값
    pub threshold: u32,
}

impl HotPathTracker {
    pub fn new() -> Self {
        Self {
            counters: HashMap::new(),
            threshold: JIT_THRESHOLD,
        }
    }

    /// 함수 실행 횟수 증가
    ///
    /// 반환값: 이 함수가 hot path인지 여부 (임계값 초과)
    pub fn record_execution(&mut self, func_id: usize) -> bool {
        let counter = self.counters.entry(func_id).or_insert(0);
        *counter += 1;
        *counter >= self.threshold
    }

    /// 함수가 이미 hot path인지 확인
    pub fn is_hot(&self, func_id: usize) -> bool {
        self.counters.get(&func_id).copied().unwrap_or(0) >= self.threshold
    }

    /// 실행 횟수 조회
    pub fn get_count(&self, func_id: usize) -> u32 {
        self.counters.get(&func_id).copied().unwrap_or(0)
    }

    /// 임계값 변경 (테스트용)
    #[cfg(test)]
    pub fn set_threshold(&mut self, threshold: u32) {
        self.threshold = threshold;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hot_path_tracker() {
        let mut tracker = HotPathTracker::new();

        // 처음에는 hot path가 아님
        assert!(!tracker.is_hot(0));
        assert_eq!(tracker.get_count(0), 0);

        // 999번 실행 - 아직 hot path 아님
        for _ in 0..999 {
            assert!(!tracker.record_execution(0));
        }
        assert!(!tracker.is_hot(0));
        assert_eq!(tracker.get_count(0), 999);

        // 1000번째 실행 - hot path!
        assert!(tracker.record_execution(0));
        assert!(tracker.is_hot(0));
        assert_eq!(tracker.get_count(0), 1000);

        // 이후에도 계속 hot path
        assert!(tracker.record_execution(0));
        assert_eq!(tracker.get_count(0), 1001);
    }

    #[test]
    fn test_hot_path_tracker_multiple_functions() {
        let mut tracker = HotPathTracker::new();
        tracker.set_threshold(10);

        // 함수 0: 5번 실행
        for _ in 0..5 {
            assert!(!tracker.record_execution(0));
        }

        // 함수 1: 15번 실행 (hot!)
        for _ in 0..15 {
            let is_hot = tracker.record_execution(1);
            if tracker.get_count(1) >= 10 {
                assert!(is_hot);
            } else {
                assert!(!is_hot);
            }
        }

        assert!(!tracker.is_hot(0));
        assert!(tracker.is_hot(1));
        assert_eq!(tracker.get_count(0), 5);
        assert_eq!(tracker.get_count(1), 15);
    }
}
