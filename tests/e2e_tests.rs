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

// Individual program tests for specific cases

#[test]
fn test_arith() {
    let path = PathBuf::from("tests/programs/arith.pyh");
    if !path.exists() {
        println!("Skipping test_arith: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(result.is_ok(), "arith.pyh should execute successfully: {:?}", result.err());
}

#[test]
fn test_fib_iter() {
    let path = PathBuf::from("tests/programs/fib_iter.pyh");
    if !path.exists() {
        println!("Skipping test_fib_iter: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(
        result.is_ok(),
        "fib_iter.pyh should execute successfully: {:?}", result.err()
    );
}

#[test]
fn test_func_rec() {
    let path = PathBuf::from("tests/programs/func_rec.pyh");
    if !path.exists() {
        println!("Skipping test_func_rec: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(
        result.is_ok(),
        "func_rec.pyh should execute successfully: {:?}", result.err()
    );
}

#[test]
fn test_loops() {
    let path = PathBuf::from("tests/programs/loops.pyh");
    if !path.exists() {
        println!("Skipping test_loops: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(result.is_ok(), "loops.pyh should execute successfully: {:?}", result.err());
}

#[test]
fn test_branch() {
    let path = PathBuf::from("tests/programs/branch.pyh");
    if !path.exists() {
        println!("Skipping test_branch: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(result.is_ok(), "branch.pyh should execute successfully: {:?}", result.err());
}

#[test]
fn test_short_circuit() {
    let path = PathBuf::from("tests/programs/short_circuit.pyh");
    if !path.exists() {
        println!("Skipping test_short_circuit: file not found");
        return;
    }

    let result = run_test_program(&path);
    assert!(
        result.is_ok(),
        "short_circuit.pyh should execute successfully: {:?}", result.err()
    );
}

