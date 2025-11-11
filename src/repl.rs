//! Pyhyeon REPL (Read-Eval-Print Loop)
//!
//! 대화형 실행 환경을 제공합니다.

use crate::vm::bytecode::{Instruction as I, Module, Value};
use crate::vm::utils::display_value;
use crate::vm::Vm;
use crate::{parse_source, analyze_with_globals};
use std::collections::HashMap;

/// REPL 세션 상태
///
/// 전역 변수, 함수, 클래스 정의를 누적하여 유지합니다.
pub struct ReplState {
    /// 누적된 전역 상태를 담은 모듈
    pub module: Module,
    /// 심볼 테이블 (이름 → 심볼 ID)
    pub symbols: HashMap<String, u16>,
    /// VM 인스턴스 (재사용)
    pub vm: Vm,
}

impl ReplState {
    /// 새 REPL 세션 생성
    pub fn new() -> Self {
        Self {
            module: Module::new(),
            symbols: HashMap::new(),
            vm: Vm::new(),
        }
    }

    /// 입력을 평가하고 실행
    ///
    /// 반환값:
    /// - Ok(Some(Value)): 표현식의 결과
    /// - Ok(None): 문장 실행 완료
    /// - Err(String): 에러 메시지
    pub fn eval_line(&mut self, input: &str) -> Result<Option<Value>, String> {
        // 입력이 비어있으면 무시
        if input.trim().is_empty() {
            return Ok(None);
        }

        // 파싱
        let program = match parse_source(input) {
            Ok(p) => p,
            Err(diagnostics) => {
                let mut error_msg = String::new();
                for diag in diagnostics {
                    error_msg.push_str(&diag.format("<repl>", input, "Parsing failed", 3));
                }
                return Err(error_msg);
            }
        };

        // 시맨틱 분석 (기존 전역 변수 포함)
        let existing_globals: Vec<String> = self.symbols.keys().cloned().collect();
        if let Err(diag) = analyze_with_globals(&program, &existing_globals) {
            return Err(diag.format("<repl>", input, "Semantic Analyzing Failed", 4));
        }

        // 컴파일 (REPL용: 기존 함수 정보 전달)
        let new_module = self.compile_with_context(&program)?;

        // 새 모듈을 기존 상태에 병합
        self.merge_module(new_module)?;

        // VM 상태 초기화
        self.vm = Vm::new();
        
        // 함수 실행
        match self.vm.run(&mut self.module) {
            Ok(ret) => Ok(ret),
            Err(err) => Err(format!("Runtime Error: {}\n{:?}", err.message, err.kind)),
        }
    }

    /// REPL용 컴파일: 기존 심볼과 함수 정보를 포함하여 컴파일
    fn compile_with_context(&self, program: &[crate::parser::ast::StmtS]) -> Result<Module, String> {
        // 기존 컨텍스트를 포함한 컴파일러 생성
        let compiler = crate::vm::Compiler::with_context(
            self.symbols.clone(),
            self.module.symbols.clone(),
            self.module.functions.clone(),
        );
        
        Ok(compiler.compile(program))
    }
    
    /// 새 모듈을 기존 상태에 병합
    ///
    /// 중요: 모든 인덱스(심볼, 함수, 문자열 등)를 재매핑해야 합니다.
    fn merge_module(&mut self, new_module: Module) -> Result<(), String> {
        // 1. 심볼 병합 및 매핑 테이블 생성
        let mut symbol_map: HashMap<u16, u16> = HashMap::new();
        for (new_idx, new_symbol) in new_module.symbols.iter().enumerate() {
            let new_idx = new_idx as u16;
            
            if let Some(&existing_idx) = self.symbols.get(new_symbol) {
                // 이미 존재하는 심볼 → 기존 인덱스 매핑
                symbol_map.insert(new_idx, existing_idx);
            } else {
                // 새 심볼 → 추가
                let old_idx = self.module.symbols.len() as u16;
                self.module.symbols.push(new_symbol.clone());
                self.module.globals.push(None); // 새 전역 변수 슬롯
                self.symbols.insert(new_symbol.clone(), old_idx);
                symbol_map.insert(new_idx, old_idx);
            }
        }

        // 2. 문자열 풀 병합
        let mut string_map: HashMap<u32, u32> = HashMap::new();
        for (new_idx, new_str) in new_module.string_pool.iter().enumerate() {
            let new_idx = new_idx as u32;
            
            if let Some(existing_idx) = self.module.string_pool.iter().position(|s| s == new_str) {
                string_map.insert(new_idx, existing_idx as u32);
            } else {
                let old_idx = self.module.string_pool.len() as u32;
                self.module.string_pool.push(new_str.clone());
                string_map.insert(new_idx, old_idx);
            }
        }

        // 3. 함수 병합 (인덱스 재매핑)
        let mut func_map: HashMap<u16, u16> = HashMap::new();
        
        for (new_idx, new_func) in new_module.functions.iter().enumerate() {
            let new_idx = new_idx as u16;
            
            // 함수 코드 내 인덱스 재매핑
            let mut remapped_func = new_func.clone();
            remapped_func.name_sym = *symbol_map.get(&new_func.name_sym)
                .ok_or_else(|| format!("Symbol mapping error for function name"))?;
            
            // __main__ 함수는 특별 처리 (항상 함수 0번에 위치)
            let func_name = &self.module.symbols[remapped_func.name_sym as usize];
            if func_name == "__main__" && new_idx == 0 {
                // 새로운 __main__을 함수 0번에 교체
                func_map.insert(0, 0);
                
                // 바이트코드 명령어 재매핑 (아직 func_map이 완전하지 않으므로 나중에 처리)
                let temp_code = new_func.code.clone();
                remapped_func.code = temp_code;
                
                // 함수 0이 없으면 추가, 있으면 교체
                if self.module.functions.is_empty() {
                    self.module.functions.push(remapped_func);
                } else {
                    self.module.functions[0] = remapped_func;
                }
            } else {
                // 일반 함수는 끝에 추가
                let old_idx = self.module.functions.len() as u16;
                func_map.insert(new_idx, old_idx);
                
                // 바이트코드 명령어는 나중에 일괄 재매핑
                remapped_func.code = new_func.code.clone();
                self.module.functions.push(remapped_func);
            }
        }
        
        // 모든 함수의 바이트코드 명령어 재매핑
        for (new_idx, _) in new_module.functions.iter().enumerate() {
            let new_idx = new_idx as u16;
            let old_idx = *func_map.get(&new_idx).unwrap();
            let old_idx_usize = old_idx as usize;
            
            if old_idx_usize < self.module.functions.len() {
                let original_code = new_module.functions[new_idx as usize].code.clone();
                self.module.functions[old_idx_usize].code = original_code.iter().map(|ins| {
                    self.remap_instruction(ins, &symbol_map, &string_map, &func_map)
                }).collect();
            }
        }

        // 4. 클래스 병합
        let mut class_map: HashMap<u16, u16> = HashMap::new();
        let base_class_idx = self.module.classes.len() as u16;
        
        for (new_idx, new_class) in new_module.classes.iter().enumerate() {
            let new_idx = new_idx as u16;
            let old_idx = base_class_idx + new_idx;
            class_map.insert(new_idx, old_idx);
            
            // 클래스의 메서드 테이블 재매핑
            let mut remapped_class = new_class.clone();
            remapped_class.methods = new_class.methods.iter().map(|(name, &func_id)| {
                let remapped_func_id = *func_map.get(&func_id)
                    .unwrap_or(&func_id);
                (name.clone(), remapped_func_id)
            }).collect();
            
            self.module.classes.push(remapped_class);
        }

        // 5. 타입 테이블 병합 (사용자 정의 타입만)
        // builtin 타입은 이미 초기화되어 있으므로 건너뜀
        for new_type in new_module.types.iter().skip(100) {
            self.module.types.push(new_type.clone());
        }

        Ok(())
    }

    /// 바이트코드 명령어의 인덱스 재매핑
    fn remap_instruction(
        &self,
        ins: &I,
        symbol_map: &HashMap<u16, u16>,
        string_map: &HashMap<u32, u32>,
        func_map: &HashMap<u16, u16>,
    ) -> I {
        match ins {
            I::ConstStr(idx) => I::ConstStr(*string_map.get(idx).unwrap_or(idx)),
            I::LoadLocal(idx) => I::LoadLocal(*idx),
            I::StoreLocal(idx) => I::StoreLocal(*idx),
            I::LoadGlobal(idx) => I::LoadGlobal(*symbol_map.get(idx).unwrap_or(idx)),
            I::StoreGlobal(idx) => I::StoreGlobal(*symbol_map.get(idx).unwrap_or(idx)),
            I::Call(func_id, argc) => I::Call(*func_map.get(func_id).unwrap_or(func_id), *argc),
            I::CallMethod(method_sym, argc) => {
                I::CallMethod(*symbol_map.get(method_sym).unwrap_or(method_sym), *argc)
            }
            I::LoadAttr(attr_sym) => I::LoadAttr(*symbol_map.get(attr_sym).unwrap_or(attr_sym)),
            I::StoreAttr(attr_sym) => I::StoreAttr(*symbol_map.get(attr_sym).unwrap_or(attr_sym)),
            // 나머지 명령어는 그대로 복사
            _ => ins.clone(),
        }
    }

    /// 결과 출력
    pub fn print_result(&self, value: &Value) {
        println!("{}", display_value(value));
    }

    /// 정의된 심볼 목록 출력
    pub fn list_symbols(&self) {
        let mut symbols: Vec<_> = self.symbols.iter().collect();
        symbols.sort_by_key(|&(_, &idx)| idx);
        
        if symbols.is_empty() {
            println!("No symbols defined.");
        } else {
            for (name, _) in symbols {
                println!("  {}", name);
            }
        }
    }

    /// 정의된 함수 목록 출력
    pub fn list_functions(&self) {
        if self.module.functions.len() <= 1 {
            println!("No functions defined.");
            return;
        }
        
        // __main__을 제외한 함수들
        for func in self.module.functions.iter().skip(1) {
            let name = &self.module.symbols[func.name_sym as usize];
            if name != "__main__" {
                println!("  {}(arity: {})", name, func.arity);
            }
        }
    }
}

impl Default for ReplState {
    fn default() -> Self {
        Self::new()
    }
}

/// 특수 명령어 처리
///
/// 반환값: true이면 REPL 종료
pub fn handle_command(cmd: &str, state: &mut ReplState) -> Result<bool, String> {
    let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
    if parts.is_empty() {
        return Ok(false);
    }

    match parts[0] {
        ":quit" | ":q" => {
            println!("Goodbye!");
            Ok(true)
        }
        ":help" | ":h" => {
            print_help();
            Ok(false)
        }
        ":clear" | ":c" => {
            *state = ReplState::new();
            println!("Session cleared.");
            Ok(false)
        }
        ":symbols" | ":s" => {
            state.list_symbols();
            Ok(false)
        }
        ":functions" | ":f" => {
            state.list_functions();
            Ok(false)
        }
        ":type" | ":t" => {
            if parts.len() < 2 {
                println!("Usage: :type <expression>");
            } else {
                let expr = parts[1..].join(" ");
                match state.eval_line(&expr) {
                    Ok(Some(val)) => {
                        let type_name = match val {
                            Value::Int(_) => "int",
                            Value::Float(_) => "float",
                            Value::Bool(_) => "bool",
                            Value::None => "NoneType",
                            Value::Object(obj) => {
                                // 타입 이름 가져오기
                                if obj.type_id < state.module.types.len() as u16 {
                                    &state.module.types[obj.type_id as usize].name
                                } else {
                                    "unknown"
                                }
                            }
                        };
                        println!("{}", type_name);
                    }
                    Ok(None) => println!("Expression returned no value"),
                    Err(e) => println!("Error: {}", e),
                }
            }
            Ok(false)
        }
        _ => Err(format!("Unknown command: {}", parts[0])),
    }
}

/// 도움말 출력
fn print_help() {
    println!(r#"Pyhyeon REPL Commands:
  :quit, :q          Exit the REPL
  :help, :h          Show this help
  :clear, :c         Clear all definitions
  :symbols, :s       List defined symbols
  :functions, :f     List defined functions
  :type <expr>, :t   Show type of expression

Tips:
  - Lines ending with ':' continue on the next line
  - Use Ctrl+C to interrupt input
  - Use arrow keys to navigate history
"#);
}

/// 입력이 계속되어야 하는지 확인
pub fn needs_more_lines(line: &str) -> bool {
    line.trim_end().ends_with(':')
}

/// 현재 버퍼가 멀티라인 모드인지 확인 (블록 내부)
pub fn is_in_block(buffer: &str) -> bool {
    if buffer.is_empty() {
        return false;
    }
    
    // 마지막 비어있지 않은 라인 찾기
    let lines: Vec<&str> = buffer.lines().collect();
    for line in lines.iter().rev() {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            // 들여쓰기가 있으면 블록 내부
            return line.starts_with("  ") || line.starts_with("\t");
        }
    }
    false
}

/// 다음 라인의 자동 들여쓰기 레벨 계산
pub fn calculate_indent(buffer: &str) -> String {
    if buffer.is_empty() {
        return String::new();
    }
    
    let lines: Vec<&str> = buffer.lines().collect();
    if lines.is_empty() {
        return String::new();
    }
    
    let last_line = lines.last().unwrap();
    
    // 마지막 라인이 ':'로 끝나면 들여쓰기 증가
    if last_line.trim_end().ends_with(':') {
        let current_indent = last_line.len() - last_line.trim_start().len();
        return " ".repeat(current_indent + 2); // 2칸 추가
    }
    
    // 현재 들여쓰기 유지
    let current_indent = last_line.len() - last_line.trim_start().len();
    if current_indent > 0 {
        return " ".repeat(current_indent);
    }
    
    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repl_state_creation() {
        let state = ReplState::new();
        assert!(state.symbols.is_empty());
        assert!(state.module.functions.is_empty());
    }

    #[test]
    fn test_simple_expression() {
        let mut state = ReplState::new();
        // 간단한 변수 할당
        let result = state.eval_line("x = 10\n");
        if let Err(e) = &result {
            eprintln!("Error in first eval: {}", e);
        }
        assert!(result.is_ok());
        
        // 변수 재사용
        let result = state.eval_line("y = x + 5\n");
        if let Err(e) = &result {
            eprintln!("Error in second eval: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_function_definition_and_call() {
        let mut state = ReplState::new();
        
        // 함수 정의
        let code = "def add(a, b):\n  return a + b\n";
        let result = state.eval_line(code);
        if let Err(e) = &result {
            eprintln!("Error in function definition: {}", e);
        }
        assert!(result.is_ok());
        
        // 함수 호출
        let result = state.eval_line("result = add(10, 20)\n");
        if let Err(e) = &result {
            eprintln!("Error in function call: {}", e);
        }
        assert!(result.is_ok());
    }

    #[test]
    fn test_multiline_detection() {
        assert!(needs_more_lines("def foo():"));
        assert!(needs_more_lines("if x > 0:"));
        assert!(needs_more_lines("while True:"));
        assert!(!needs_more_lines("x = 10"));
        assert!(!needs_more_lines("print('hello')"));
    }

    #[test]
    fn test_error_recovery() {
        let mut state = ReplState::new();
        
        // 구문 에러
        let result = state.eval_line("x = = 10\n");
        assert!(result.is_err());
        
        // 에러 후에도 상태가 유지되어야 함
        let result = state.eval_line("x = 10\n");
        assert!(result.is_ok());
    }

    #[test]
    fn test_module_merge() {
        let mut state = ReplState::new();
        
        // 첫 번째 변수
        let _ = state.eval_line("a = 1\n");
        assert!(state.symbols.contains_key("a"));
        
        // 두 번째 변수
        let _ = state.eval_line("b = 2\n");
        assert!(state.symbols.contains_key("a"));
        assert!(state.symbols.contains_key("b"));
        
        // 기존 변수 재할당
        let _ = state.eval_line("a = 100\n");
        assert!(state.symbols.contains_key("a"));
    }
}

