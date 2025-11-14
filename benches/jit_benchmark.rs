use pyhyeon::{Compiler, Module, Vm};
use std::time::Instant;

fn main() {
    // 테스트 프로그램: Fibonacci
    let source = r#"
def fib(n):
  if n < 2:
    return n
  return fib(n - 1) + fib(n - 2)

# 2000번 호출 (처음 1000번은 인터프리터, 이후 JIT)
result = 0
for i in range(2000):
  result = fib(20)

print(result)
"#;

    println!("=== Pyhyeon JIT Benchmark ===\n");
    println!("Test: Fibonacci(20) called 2000 times");
    println!("Expected behavior:");
    println!("  - First 1000 calls: Interpreter mode");
    println!("  - After 1000 calls: JIT compilation triggered");
    println!("  - Remaining calls: Native code execution\n");

    // 컴파일
    println!("Compiling...");
    let mut compiler = Compiler::new();
    let ast = match pyhyeon::parse(source) {
        Ok(ast) => ast,
        Err(e) => {
            eprintln!("Parse error: {:?}", e);
            return;
        }
    };

    let mut module = Module::new();
    if let Err(e) = compiler.compile(&ast, &mut module) {
        eprintln!("Compilation error: {:?}", e);
        return;
    }

    println!("Compilation successful!\n");

    // 실행 및 시간 측정
    println!("Running benchmark...");
    let start = Instant::now();

    let mut vm = Vm::new();
    match vm.run(&mut module) {
        Ok(_) => {
            let elapsed = start.elapsed();
            println!("\n=== Results ===");
            println!("Total execution time: {:.2}ms", elapsed.as_secs_f64() * 1000.0);
            println!("Average time per call: {:.4}ms", elapsed.as_secs_f64() * 1000.0 / 2000.0);
        }
        Err(e) => {
            eprintln!("Runtime error: {:?}", e);
        }
    }
}
