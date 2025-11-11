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

    // Recursively scan all subdirectories
    fn scan_dir(dir: &PathBuf, programs: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    scan_dir(&path, programs);
                } else if path.extension().and_then(|s| s.to_str()) == Some("pyh") {
                    programs.push(path);
                }
            }
        }
    }

    scan_dir(&test_dir, &mut programs);
    programs.sort();
    programs
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
        let name = path.strip_prefix("tests/programs/").unwrap_or(&path).to_string_lossy();
        print!("Testing {}... ", name);

        match run_test_program_with_input(&path, &[]) {
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

/// Improved test macro with better ergonomics
///
/// Usage:
/// ```
/// // Inside test function body
/// assert_program!("basics/arith.pyh");
/// assert_program!("io/input_with_prompt.pyh", inputs = ["Alice"], expects = ["Hello, Alice"]);
/// assert_program!("basics/fib_iter.pyh", expects = ["55"]);
///
/// // Generate test function directly
/// assert_program!(test_name_here, "basics/arith.pyh");
/// assert_program!(test_io_custom, "io/input_with_prompt.pyh", inputs = ["Alice"], expects = ["Hello"]);
/// ```
macro_rules! assert_program {
    // Generate test function: simple case
    ($test_name:ident, $path:literal) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let result = run_test_program_with_input(&path, &[]);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $path,
                result.err()
            );
        }
    };

    // Generate test function: with inputs only
    ($test_name:ident, $path:literal, inputs = [$($input:literal),* $(,)?]) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $path,
                result.err()
            );
        }
    };

    // Generate test function: with expects only
    ($test_name:ident, $path:literal, expects = [$($expected:literal),* $(,)?]) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let result = run_test_program_with_input(&path, &[]);

            match result {
                Ok(output) => {
                    $(
                        assert!(
                            output.contains($expected),
                            "{}: output should contain '{}'\nActual output:\n{}",
                            $path,
                            $expected,
                            output
                        );
                    )*
                }
                Err(err) => {
                    panic!("{} should execute successfully: {:?}", $path, err);
                }
            }
        }
    };

    // Generate test function: with both inputs and expects
    ($test_name:ident, $path:literal, inputs = [$($input:literal),* $(,)?], expects = [$($expected:literal),* $(,)?]) => {
        #[test]
        fn $test_name() {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);

            match result {
                Ok(output) => {
                    $(
                        assert!(
                            output.contains($expected),
                            "{}: output should contain '{}'\nActual output:\n{}",
                            $path,
                            $expected,
                            output
                        );
                    )*
                }
                Err(err) => {
                    panic!("{} should execute successfully: {:?}", $path, err);
                }
            }
        }
    };

    // ============================================================================
    // Inline assertion variants (for use inside existing test functions)
    // ============================================================================

    // Simple case: just run the program
    ($path:literal) => {
        {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let result = run_test_program_with_input(&path, &[]);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $path,
                result.err()
            );
        }
    };

    // With inputs only
    ($path:literal, inputs = [$($input:literal),* $(,)?]) => {
        {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);
            assert!(
                result.is_ok(),
                "{} should execute successfully: {:?}",
                $path,
                result.err()
            );
        }
    };

    // With expects only
    ($path:literal, expects = [$($expected:literal),* $(,)?]) => {
        {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let result = run_test_program_with_input(&path, &[]);

            match result {
                Ok(output) => {
                    $(
                        assert!(
                            output.contains($expected),
                            "{}: output should contain '{}'\nActual output:\n{}",
                            $path,
                            $expected,
                            output
                        );
                    )*
                }
                Err(err) => {
                    panic!("{} should execute successfully: {:?}", $path, err);
                }
            }
        }
    };

    // With both inputs and expects
    ($path:literal, inputs = [$($input:literal),* $(,)?], expects = [$($expected:literal),* $(,)?]) => {
        {
            let _ = include_str!(concat!("programs/", $path));
            let path = PathBuf::from(concat!("tests/programs/", $path));
            let inputs = vec![$($input),*];
            let result = run_test_program_with_input(&path, &inputs);

            match result {
                Ok(output) => {
                    $(
                        assert!(
                            output.contains($expected),
                            "{}: output should contain '{}'\nActual output:\n{}",
                            $path,
                            $expected,
                            output
                        );
                    )*
                }
                Err(err) => {
                    panic!("{} should execute successfully: {:?}", $path, err);
                }
            }
        }
    };
}

// ============================================================================
// Basic Tests - 기본 기능 (산술, 분기, 재귀, short-circuit 등)
// ============================================================================

assert_program!(test_basics_arithmetic, "basics/arith.pyh", expects = ["7"]);
assert_program!(test_basics_branching, "basics/branch.pyh", expects = ["-1", "0", "1"]);
assert_program!(test_basics_fibonacci, "basics/fib_iter.pyh");
assert_program!(test_basics_recursion, "basics/func_rec.pyh", expects = ["720"]);
assert_program!(test_basics_short_circuit, "basics/short_circuit.pyh", expects = ["False", "True"]);
assert_program!(test_basics_float, "basics/test_float.pyh", expects = ["3.14", "42", "2.5"]);
assert_program!(test_basics_edge_cases, "basics/edge_case_comprehensive.pyh");

// ============================================================================
// Loop Tests - 반복문 (for, while, break, continue, 중첩)
// ============================================================================

assert_program!(test_loops_basic, "loops/loops.pyh", expects = ["15"]);
assert_program!(test_loops_comprehensive, "loops/loop_comprehensive.pyh", expects = [
    "=== Basic for loops ===",
    "10",
    "20",
    "=== While with break/continue ===",
    "=== For with break/continue ===",
    "=== Nested loops ===",
    "=== Loops in function ===",
    "120",
    "=== Complex nested - primes ===",
    "8",
    "=== All tests complete ==="
]);

// ============================================================================
// Collection Tests - 리스트와 딕셔너리
// ============================================================================

assert_program!(test_collections_list_basic, "collections/list_basic.pyh", expects = ["[1, 2, 3]", "1", "3"]);
assert_program!(test_collections_list_methods, "collections/list_methods.pyh", expects = ["[1, 2, 3, 4]", "4", "[1, 2, 3]"]);
assert_program!(test_collections_list_iteration, "collections/list_for.pyh", expects = ["1", "2", "3", "4", "5", "Done"]);
assert_program!(test_collections_dict_basic, "collections/dict_basic.pyh");
assert_program!(test_collections_dict_methods, "collections/dict_methods.pyh");
assert_program!(test_collections_dict_iteration, "collections/dict_for.pyh", expects = ["Done"]);
assert_program!(test_collections_complex, "collections/collections_complex.pyh", expects = ["[[1, 2], [3, 4]]", "Alice", "Bob"]);
assert_program!(test_collections_list_comprehension_alt, "collections/list_comprehension_alt.pyh", expects = ["[0, 2, 4]", "[0, 1, 4, 9, 16]"]);

// ============================================================================
// Class Tests - 클래스, 객체, 메서드
// ============================================================================

assert_program!(test_classes_basic, "classes/class_basic.pyh", expects = ["3", "4"]);
assert_program!(test_classes_creation, "classes/class_create.pyh");
assert_program!(test_classes_simple, "classes/class_simple.pyh");
assert_program!(test_classes_method_chaining, "classes/method_chaining.pyh", expects = ["Hello World"]);

// ============================================================================
// I/O Tests - 입출력 (input/output)
// ============================================================================

assert_program!(test_io_input_with_prompt, "io/input_with_prompt.pyh",
    inputs = ["철수"],
    expects = ["이름을 입력하세요: ", "안녕하세요, 철수님!"]);

assert_program!(test_io_input_without_prompt, "io/input_without_prompt.pyh",
    inputs = ["25"],
    expects = ["나이: 25"]);

assert_program!(test_io_input_multiple, "io/input_multiple.pyh",
    inputs = ["Alice", "30"],
    expects = ["Name: ", "Age: ", "Alice is 30 years old"]);

assert_program!(test_io_input_int_conversion, "io/input_int_conversion.pyh",
    inputs = ["10", "20"],
    expects = ["Enter a number: ", "Enter another number: ", "Sum: 30"]);

assert_program!(test_io_input_in_loop, "io/input_in_loop.pyh",
    inputs = ["Alice", "Bob", "Charlie"],
    expects = ["Hello, Alice", "Hello, Bob", "Hello, Charlie"]);

// ============================================================================
// String Tests - 문자열 조작
// ============================================================================

assert_program!(test_strings_basics, "strings/string_basics.pyh", expects = ["hello", "world", "hello world"]);
assert_program!(test_strings_advanced, "strings/string_advanced.pyh", expects = ["42", "5", "hello world"]);
assert_program!(test_strings_methods, "strings/string_methods.pyh", expects = ["HELLO WORLD", "hello world", "spaces"]);
