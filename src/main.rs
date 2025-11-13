use pyhyeon as lib;
use std::env;

#[cfg(not(target_arch = "wasm32"))]
use rustyline::DefaultEditor;
#[cfg(not(target_arch = "wasm32"))]
use rustyline::error::ReadlineError;

fn main() {
    // Subcommands: repl/run/compile/exec/disasm/dism
    // repl: start the REPL (DEFAULT)
    // run <file>: compile and execute the program
    // compile <file> -o <output file>: compile the program to bytecode file
    // exec <file>: execute the bytecode file
    // disasm <file>: disassemble the bytecode file and print the result to the console
    // dism <file>: compile source file and disassemble
    let mut args = env::args().skip(1).collect::<Vec<String>>();
    let mut subcmd = "repl".to_string();
    let mut input_path = "./test.pyh".to_string();
    let mut out_path: Option<String> = None;
    if !args.is_empty() {
        let first = &args[0];
        if ["run", "compile", "exec", "repl", "disasm", "dism"].contains(&first.as_str()) {
            subcmd = first.clone();
            args.remove(0);
        }
    }
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "-o" => {
                if i + 1 < args.len() {
                    out_path = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            p => {
                input_path = p.to_string();
            }
        }
        i += 1;
    }

    match subcmd.as_str() {
        "repl" => {
            #[cfg(not(target_arch = "wasm32"))]
            {
                if let Err(e) = run_repl() {
                    eprintln!("REPL Error: {}", e);
                }
                return;
            }
            #[cfg(target_arch = "wasm32")]
            {
                eprintln!("REPL is not available in WASM builds");
                return;
            }
        }
        _ => {}
    }

    // For file-based commands, read the source file
    let path = input_path.as_str();
    let src = std::fs::read_to_string(path).expect("Failed to read source file");
    // add newline if not present at the end of file
    let src = if src.ends_with('\n') {
        src
    } else {
        format!("{}\n", src)
    };

    match subcmd.as_str() {
        "run" => {
            let program = match lib::parse_source(&src) {
                Ok(p) => p,
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.format(path, &src, "Parsing failed", 3));
                    }
                    return;
                }
            };
            if let Err(diag) = lib::analyze(&program) {
                eprint!(
                    "{}",
                    diag.format(path, &src, "Semantic Analyzing Failed", 4)
                );
                return;
            }
            // VM only
            let module = lib::compile_to_module(&program);
            lib::exec_vm_module(module);
        }
        "compile" => {
            let program = match lib::parse_source(&src) {
                Ok(p) => p,
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.format(path, &src, "Parsing failed", 3));
                    }
                    return;
                }
            };
            if let Err(diag) = lib::analyze(&program) {
                eprint!(
                    "{}",
                    diag.format(path, &src, "Semantic Analyzing Failed", 4)
                );
                return;
            }
            let module = lib::compile_to_module(&program);
            let out = out_path.as_deref().unwrap_or("out.pyhb");
            lib::save_module(&module, out).expect("failed to save module");
            println!("wrote {}", out);
        }
        "dism" => {
            let program = match lib::parse_source(&src) {
                Ok(p) => p,
                Err(diagnostics) => {
                    for diag in diagnostics {
                        eprint!("{}", diag.format(path, &src, "Parsing failed", 3));
                    }
                    return;
                }
            };
            if let Err(diag) = lib::analyze(&program) {
                eprint!(
                    "{}",
                    diag.format(path, &src, "Semantic Analyzing Failed", 4)
                );
                return;
            }
            let module = lib::compile_to_module(&program);
            let output = lib::vm::disasm::disassemble_module_to_string(&module);
            print!("{}", output);
        }
        "disasm" => {
            let module = lib::load_module(path).expect("failed to load module");
            let output = lib::vm::disasm::disassemble_module_to_string(&module);
            print!("{}", output);
        }
        "exec" => {
            let module = lib::load_module(path).expect("failed to load module");
            lib::exec_vm_module(module);
        }
        _ => {
            eprintln!("Unknown subcommand: {}", subcmd);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn run_repl() -> Result<(), String> {
    println!("Pyhyeon REPL");
    println!("Type :help for help, :quit to exit\n");

    // rustyline 에디터 초기화
    let mut rl = DefaultEditor::new().map_err(|e| format!("Failed to create editor: {}", e))?;

    // 히스토리 파일 경로
    let history_path = dirs::home_dir()
        .map(|h| h.join(".pyhyeon_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".pyhyeon_history"));

    // 히스토리 로드
    if history_path.exists() {
        let _ = rl.load_history(&history_path);
    }

    // REPL 상태 초기화
    let mut repl_state = lib::repl::ReplState::new();
    let mut buffer = String::new();
    let mut in_block = false;

    loop {
        // 프롬프트 설정
        let prompt = if buffer.is_empty() { ">>> " } else { "... " };

        // 라인 읽기
        match rl.readline(prompt) {
            Ok(mut line) => {
                // 특수 명령어 처리 (버퍼가 비어있을 때만)
                if line.trim().starts_with(':') && buffer.is_empty() {
                    match lib::repl::handle_command(&line, &mut repl_state) {
                        Ok(should_quit) => {
                            if should_quit {
                                break;
                            }
                        }
                        Err(e) => eprintln!("{}", e),
                    }
                    continue;
                }

                // 멀티라인 모드에서 빈 라인 입력 시 실행
                if in_block && line.trim().is_empty() {
                    in_block = false;

                    // 히스토리에 전체 블록 추가
                    let _ = rl.add_history_entry(&buffer);

                    // 실행
                    match repl_state.eval_line(&buffer) {
                        Ok(Some(value)) => {
                            repl_state.print_result(&value);
                        }
                        Ok(None) => {
                            // 문장 실행 완료 (출력 없음)
                        }
                        Err(e) => {
                            eprintln!("{}", e);
                        }
                    }

                    buffer.clear();
                    continue;
                }

                // 자동 들여쓰기가 있는 경우 라인 앞에 추가
                if in_block && !line.trim().is_empty() {
                    // 이미 들여쓰기가 있으면 그대로, 없으면 현재 들여쓰기 유지
                    if !line.starts_with(' ') && !line.starts_with('\t') {
                        let indent = lib::repl::calculate_indent(&buffer);
                        line = format!("{}{}", indent, line);
                    }
                }

                // 버퍼에 추가
                buffer.push_str(&line);
                buffer.push('\n');

                // 히스토리에 개별 라인 추가 (블록 아닐 때만)
                if !in_block {
                    let _ = rl.add_history_entry(&line);
                }

                // 멀티라인 모드 진입/유지 체크
                if lib::repl::needs_more_lines(&line) {
                    in_block = true;
                    continue;
                }

                // 블록 내부라면 계속
                if in_block {
                    continue;
                }

                // 단일 라인 실행
                match repl_state.eval_line(&buffer) {
                    Ok(Some(value)) => {
                        repl_state.print_result(&value);
                    }
                    Ok(None) => {
                        // 문장 실행 완료 (출력 없음)
                    }
                    Err(e) => {
                        eprintln!("{}", e);
                    }
                }

                buffer.clear();
            }
            Err(ReadlineError::Interrupted) => {
                // Ctrl+C
                println!("^C");
                buffer.clear();
                in_block = false;
            }
            Err(ReadlineError::Eof) => {
                // Ctrl+D
                println!("exit");
                break;
            }
            Err(err) => {
                eprintln!("Error: {:?}", err);
                break;
            }
        }
    }

    // 히스토리 저장
    let _ = rl.save_history(&history_path);

    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn run_repl() -> Result<(), String> {
    Err("REPL is not available in WASM builds".to_string())
}
