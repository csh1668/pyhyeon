use pyhyeon;
use pyhyeon::runtime_io::BufferIo;
use std::fs;
use std::path::PathBuf;

/// E2E 통합 테스트: tests/programs/ 디렉터리의 모든 .pyh 파일을
/// VM으로 실행하여 정상 동작을 확인합니다.

fn get_test_programs() -> Vec<PathBuf> {
    let test_dir = PathBuf::from("tests/programs");
    if !test_dir.exists() {
        return vec![];
    }

    let mut programs = vec![];
    if let Ok(entries) = fs::read_dir(&test_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("pyh") {
                programs.push(path);
            }
        }
    }
    programs.sort();
    programs
}

fn run_test_program(path: &PathBuf) -> Result<String, String> {
    run_test_program_with_input(path, &[])
}

fn run_test_program_with_input(path: &PathBuf, inputs: &[&str]) -> Result<String, String> {
    let source = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let path_str = path.to_string_lossy().to_string();

    // Parse
    let program = pyhyeon::parse_source(&source).map_err(|diagnostics| {
        format!(
            "Parse error in {}: {}",
            path_str,
            diagnostics
                .iter()
                .map(|d| d.format(&path_str, &source, "Parse Error", 0))
                .collect::<String>()
        )
    })?;

    // Analyze
    pyhyeon::analyze(&program).map_err(|diag| {
        format!(
            "Semantic error in {}: {}",
            path_str,
            diag.format(&path_str, &source, "Semantic Error", 0)
        )
    })?;

    // Compile and run with VM (capture output)
    let mut module = pyhyeon::compile_to_module(&program);
    let mut vm = pyhyeon::Vm::new();
    let mut vm_io = BufferIo::new();

    // Add input lines
    for input in inputs {
        vm_io.push_input_line(*input);
    }

    if let Err(err) = vm.run_with_io(&mut module, &mut vm_io) {
        return Err(format!("VM error in {}: {:?}", path_str, err));
    }
    let vm_output = vm_io.take_output();

    Ok(vm_output)
}


#[test]
fn test_e2e_all_programs() {
    let programs = get_test_programs();

    if programs.is_empty() {
        println!("Warning: No test programs found in tests/programs/");
        return;
    }

    let mut passed = 0;
    let mut failed = 0;

    for path in programs {
        let name = path.file_name().unwrap().to_string_lossy();
        print!("Testing {}... ", name);

        match run_test_program(&path) {
            Ok(_vm_out) => {
                println!("✓ PASSED");
                passed += 1;
            }
            Err(err) => {
                println!("✗ ERROR: {}", err);
                failed += 1;
            }
        }
    }

    println!("\n========================================");
    println!("E2E Test Summary:");
    println!("  Passed: {}", passed);
    println!("  Failed: {}", failed);
    println!("  Total:  {}", passed + failed);
    println!("========================================");

    assert_eq!(failed, 0, "Some E2E tests failed");
}

macro_rules! test_program {
    ($test_name:ident, $filename:literal) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $filename));
            
            let path = PathBuf::from(concat!("tests/programs/", $filename));
            let result = run_test_program(&path);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $filename,
                result.err()
            );
        }
    };

    ($test_name:ident, $filename:literal, inputs = [$($input:literal),*]) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $filename));
            
            let path = PathBuf::from(concat!("tests/programs/", $filename));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $filename,
                result.err()
            );
        }
    };

    ($test_name:ident, $filename:literal, inputs = [$($input:literal),*], contains = [$($expected:literal),*]) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $filename));
            
            let path = PathBuf::from(concat!("tests/programs/", $filename));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);
            
            match result {
                Ok(output) => {
                    $(
                        assert!(
                            output.contains($expected),
                            "{}: output should contain '{}'\nActual output:\n{}",
                            $filename,
                            $expected,
                            output
                        );
                    )*
                }
                Err(err) => {
                    panic!("{} should execute successfully: {:?}", $filename, err);
                }
            }
        }
    };
}

// Generate tests for each program
test_program!(test_arith, "arith.pyh");
test_program!(test_fib_iter, "fib_iter.pyh");
test_program!(test_func_rec, "func_rec.pyh");
test_program!(test_loops, "loops.pyh");
test_program!(test_branch, "branch.pyh");
test_program!(test_short_circuit, "short_circuit.pyh");
test_program!(test_string_basics, "string_basics.pyh");
test_program!(test_string_advanced, "string_advanced.pyh");
test_program!(
    test_input_with_prompt,
    "input_with_prompt.pyh",
    inputs = ["철수"],
    contains = ["이름을 입력하세요: ", "안녕하세요, 철수님!"]
);
test_program!(
    test_input_without_prompt,
    "input_without_prompt.pyh",
    inputs = ["25"],
    contains = ["나이: 25"]
);
test_program!(
    test_input_multiple,
    "input_multiple.pyh",
    inputs = ["Alice", "30"],
    contains = ["Name: ", "Age: ", "Alice is 30 years old"]
);
test_program!(
    test_input_int_conversion,
    "input_int_conversion.pyh",
    inputs = ["10", "20"],
    contains = ["Enter a number: ", "Enter another number: ", "Sum: 30"]
);
test_program!(
    test_input_in_loop,
    "input_in_loop.pyh",
    inputs = ["Alice", "Bob", "Charlie"],
    contains = ["Hello, Alice", "Hello, Bob", "Hello, Charlie"]
);
