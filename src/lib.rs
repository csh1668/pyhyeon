pub mod builtins;
pub mod lexer;
pub mod parser;
#[cfg(not(target_arch = "wasm32"))]
pub mod repl;
pub mod runtime_io;
pub mod semantic;
pub mod types;
pub mod vm;

pub use runtime_io::RuntimeIo;
pub use vm::Vm;

use ariadne::{Color, Label, Report, ReportKind, Source};
use chumsky::Parser;
use chumsky::input::{Input, Stream};
use chumsky::span::SimpleSpan;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub message: String,
    pub span: std::ops::Range<usize>,
}

impl Diagnostic {
    pub fn format(&self, path: &str, src: &str, kind: &str, code: usize) -> String {
        let mut buffer = Vec::new();
        Report::build(ReportKind::Error, (path, self.span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_code(code)
            .with_message(kind)
            .with_label(
                Label::new((path, self.span.clone()))
                    .with_message(&self.message)
                    .with_color(Color::Red),
            )
            .finish()
            .write((path, Source::from(src)), &mut buffer)
            .ok();
        String::from_utf8_lossy(&buffer).to_string()
    }
}

pub fn parse_source(src: &str) -> Result<Vec<parser::ast::StmtS>, Vec<Diagnostic>> {
    let mut lexer = lexer::Lexer::new(src);

    // First pass: collect all tokens and check for lexer errors
    let mut tokens = Vec::new();
    let mut lexer_errors = Vec::new();

    loop {
        let (t, span) = lexer.next_token_with_span();

        // Check for lexer errors
        if let lexer::token::Token::Error(msg, error_span) = t {
            lexer_errors.push(Diagnostic {
                message: msg,
                span: error_span,
            });
            continue; // Skip error tokens
        }

        if t == lexer::token::Token::Eof {
            break;
        }

        tokens.push((t, SimpleSpan::new(span.start, span.end)));
    }

    // If there are lexer errors, return them immediately
    if !lexer_errors.is_empty() {
        return Err(lexer_errors);
    }

    let eoi_span = parser::SimpleSpan::new(src.len(), src.len());
    let token_stream = Stream::from_iter(tokens).map(eoi_span, |(t, s)| (t, s));
    match parser::program_parser().parse(token_stream).into_result() {
        Ok(program) => Ok(program),
        Err(errors) => {
            let mut errors = errors;
            errors.sort_by(|x1, x2| {
                let x1 = (x1.span().start, x1.span().end);
                let x2 = (x2.span().start, x2.span().end);
                x1.cmp(&x2)
            });
            let diagnostics = errors
                .into_iter()
                .map(|e| Diagnostic {
                    message: e.reason().to_string(),
                    span: e.span().into_range(),
                })
                .collect();
            Err(diagnostics)
        }
    }
}

pub fn analyze(program: &[parser::ast::StmtS]) -> Result<(), Diagnostic> {
    match semantic::analyze(program) {
        Ok(_) => Ok(()),
        Err(e) => Err(Diagnostic {
            message: e.message,
            span: e.span,
        }),
    }
}

pub fn analyze_with_globals(
    program: &[parser::ast::StmtS],
    existing_globals: &[String],
) -> Result<(), Diagnostic> {
    match semantic::analyze_with_globals(program, existing_globals) {
        Ok(_) => Ok(()),
        Err(e) => Err(Diagnostic {
            message: e.message,
            span: e.span,
        }),
    }
}

pub fn compile_to_module(program: &[parser::ast::StmtS]) -> vm::bytecode::Module {
    let compiler = vm::Compiler::new();
    compiler.compile(program)
}

pub fn exec_vm_module(mut module: vm::bytecode::Module) {
    let mut machine = vm::Vm::new();
    if let Err(err) = machine.run(&mut module) {
        eprintln!("VM Runtime Error: {:?}: {}", err.kind, err.message);
    }
}

pub fn save_module(module: &vm::bytecode::Module, path: &str) -> std::io::Result<()> {
    let cfg = bincode::config::standard();
    let bytes = bincode::serde::encode_to_vec(module, cfg).expect("serialize module");
    std::fs::write(path, bytes)
}

pub fn load_module(path: &str) -> std::io::Result<vm::bytecode::Module> {
    let bytes = std::fs::read(path)?;
    let cfg = bincode::config::standard();
    let (module, _consumed): (vm::bytecode::Module, usize) =
        bincode::serde::decode_from_slice(&bytes, cfg).expect("deserialize module");
    Ok(module)
}

// ===== wasm_bindgen exports for web playground =====
#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use super::*;
    use serde::Serialize;
    use std::cell::RefCell;
    use wasm_bindgen::prelude::*;

    /// VM execution session
    struct VmSession {
        vm: vm::Vm,
        module: vm::bytecode::Module,
        io: runtime_io::BufferIo,
        execution_timer: Option<instant::Instant>,
        accumulated_time: std::time::Duration,
    }

    thread_local! {
        static ACTIVE_SESSION: RefCell<Option<VmSession>> = RefCell::new(None);
    }

    #[derive(Serialize)]
    pub struct WasmDiagnostic {
        pub message: String,
        pub start_line: u32,
        pub start_char: u32,
        pub end_line: u32,
        pub end_char: u32,
        pub severity: u8,
    }

    #[derive(Serialize)]
    pub struct VmStateInfo {
        pub state: String, // "running", "waiting_for_input", "finished", "error"
        pub output: String,
        pub execution_time_ms: Option<f64>,
    }

    fn byte_to_lc(src: &str, byte_idx: usize) -> (u32, u32) {
        let prefix = &src[..byte_idx.min(src.len())];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
        let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = src[line_start..byte_idx.min(src.len())]
            .encode_utf16()
            .count() as u32;
        (line, col)
    }

    #[wasm_bindgen]
    pub fn analyze(src: &str) -> JsValue {
        // parse
        let program = match super::parse_source(src) {
            Ok(p) => p,
            Err(diagnostics) => {
                let wasm_diags: Vec<WasmDiagnostic> = diagnostics
                    .iter()
                    .map(|d| {
                        let (sl, sc) = byte_to_lc(src, d.span.start);
                        let (el, ec) = byte_to_lc(src, d.span.end);
                        WasmDiagnostic {
                            message: d.message.clone(),
                            start_line: sl,
                            start_char: sc,
                            end_line: el,
                            end_char: ec,
                            severity: 1,
                        }
                    })
                    .collect();
                return serde_wasm_bindgen::to_value(&wasm_diags).unwrap();
            }
        };
        // semantic
        if let Err(diag) = super::analyze(&program) {
            let (sl, sc) = byte_to_lc(src, diag.span.start);
            let (el, ec) = byte_to_lc(src, diag.span.end);
            let wasm_diag = WasmDiagnostic {
                message: diag.message,
                start_line: sl,
                start_char: sc,
                end_line: el,
                end_char: ec,
                severity: 1,
            };
            return serde_wasm_bindgen::to_value(&vec![wasm_diag]).unwrap();
        }
        serde_wasm_bindgen::to_value(&Vec::<WasmDiagnostic>::new()).unwrap()
    }

    /// Compile and execute via VM, capturing output; return it as a string.
    #[wasm_bindgen]
    pub fn run(src: &str) -> String {
        let start_time = instant::Instant::now();
        let src = if src.ends_with('\n') {
            src.to_string()
        } else {
            format!("{}\n", src)
        };

        // Parse
        let program = match super::parse_source(&src) {
            Ok(p) => p,
            Err(diagnostics) => {
                let mut output = String::new();
                for diag in diagnostics {
                    output.push_str(&diag.format("<mem>", &src, "Parsing failed", 3));
                }
                return output;
            }
        };

        // Semantic analysis
        if let Err(diag) = super::analyze(&program) {
            return diag.format("<mem>", &src, "Semantic Analyzing Failed", 4);
        }

        // Compile and run
        let module = super::compile_to_module(&program);
        let exec_start = instant::Instant::now();
        let mut vm = super::vm::Vm::new();
        let mut io = super::runtime_io::BufferIo::new();
        let result = match vm.run_with_io(&mut module.clone(), &mut io) {
            Ok(_) => {
                let exec_time = exec_start.elapsed();
                let total_time = start_time.elapsed();
                let output = io.take_output();
                if output.is_empty() {
                    format!(
                        "\n────────────────────────────────────\nExecution time: {:.3}ms\nTotal time: {:.3}ms",
                        exec_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0
                    )
                } else {
                    format!(
                        "{}\n────────────────────────────────────\nExecution time: {:.3}ms\nTotal time: {:.3}ms",
                        output.trim_end(),
                        exec_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0
                    )
                }
            }
            Err(err) => format!("Runtime Error: {}\n{:?}", err.message, err.kind),
        };
        result
    }

    /// Start a new program execution (interactive mode)
    #[wasm_bindgen]
    pub fn start_program(src: &str) -> JsValue {
        let src = if src.ends_with('\n') {
            src.to_string()
        } else {
            format!("{}\n", src)
        };

        // Parse
        let program = match super::parse_source(&src) {
            Ok(p) => p,
            Err(diagnostics) => {
                let mut output = String::new();
                for diag in diagnostics {
                    output.push_str(&diag.format("<mem>", &src, "Parsing failed", 3));
                }
                return serde_wasm_bindgen::to_value(&VmStateInfo {
                    state: "error".to_string(),
                    output,
                    execution_time_ms: None,
                })
                .unwrap();
            }
        };

        // Semantic analysis
        if let Err(diag) = super::analyze(&program) {
            return serde_wasm_bindgen::to_value(&VmStateInfo {
                state: "error".to_string(),
                output: diag.format("<mem>", &src, "Semantic Analyzing Failed", 4),
                execution_time_ms: None,
            })
            .unwrap();
        }

        // Compile
        let module = super::compile_to_module(&program);
        let vm = super::vm::Vm::new();
        let io = super::runtime_io::BufferIo::new();

        // Start execution
        let session = VmSession {
            vm,
            module,
            io,
            execution_timer: Some(instant::Instant::now()),
            accumulated_time: std::time::Duration::from_secs(0),
        };
        ACTIVE_SESSION.with(|s| {
            *s.borrow_mut() = Some(session);
        });

        // Run until we need input or finish
        step_program()
    }

    /// Continue program execution (step)
    #[wasm_bindgen]
    pub fn step_program() -> JsValue {
        ACTIVE_SESSION.with(|s| {
            let mut session_opt = s.borrow_mut();
            if let Some(ref mut session) = *session_opt {
                // Start timer if not running
                if session.execution_timer.is_none() {
                    session.execution_timer = Some(instant::Instant::now());
                }

                // Execute
                match session.vm.run_with_io(&mut session.module, &mut session.io) {
                    Ok(_) => {
                        let state = session.vm.get_state();

                        // Stop timer and accumulate if waiting for input or finished
                        let mut execution_time_ms = None;
                        let is_waiting = state == vm::machine::VmState::WaitingForInput;
                        let is_finished = state == vm::machine::VmState::Finished;
                        let is_error = state == vm::machine::VmState::Error;

                        if is_waiting || is_finished || is_error {
                            if let Some(timer) = session.execution_timer.take() {
                                session.accumulated_time += timer.elapsed();
                            }

                            // Set execution time for finished state
                            if is_finished {
                                execution_time_ms =
                                    Some(session.accumulated_time.as_secs_f64() * 1000.0);
                            }
                        }

                        let state_str = vm_state_to_string(state);

                        serde_wasm_bindgen::to_value(&VmStateInfo {
                            state: state_str.to_string(),
                            output: session.io.drain_output(),
                            execution_time_ms,
                        })
                        .unwrap()
                    }
                    Err(err) => {
                        // Stop timer on error
                        if let Some(timer) = session.execution_timer.take() {
                            session.accumulated_time += timer.elapsed();
                        }

                        // Get previous output and append error message with red color
                        let previous_output = session.io.drain_output();
                        let error_msg = format!(
                            "\x1b[31mRuntime Error: {}\n{:?}\x1b[0m",
                            err.message, err.kind
                        );
                        let combined_output = if previous_output.is_empty() {
                            error_msg
                        } else {
                            format!("{}\n{}", previous_output, error_msg)
                        };

                        serde_wasm_bindgen::to_value(&VmStateInfo {
                            state: "error".to_string(),
                            output: combined_output,
                            execution_time_ms: None,
                        })
                        .unwrap()
                    }
                }
            } else {
                serde_wasm_bindgen::to_value(&VmStateInfo {
                    state: "error".to_string(),
                    output: "No active program".to_string(),
                    execution_time_ms: None,
                })
                .unwrap()
            }
        })
    }

    /// Provide input to the running program
    #[wasm_bindgen]
    pub fn provide_input(line: &str) -> JsValue {
        ACTIVE_SESSION.with(|s| {
            let mut session_opt = s.borrow_mut();
            if let Some(ref mut session) = *session_opt {
                // Add input to the buffer
                session.io.push_input_line(line.to_string());
                // Resume execution
                session.vm.resume();

                // Restart timer before execution
                session.execution_timer = Some(instant::Instant::now());

                // Continue execution
                match session.vm.run_with_io(&mut session.module, &mut session.io) {
                    Ok(_) => {
                        let state = session.vm.get_state();

                        // Stop timer and accumulate if waiting for input or finished
                        let mut execution_time_ms = None;
                        let is_waiting = state == vm::machine::VmState::WaitingForInput;
                        let is_finished = state == vm::machine::VmState::Finished;
                        let is_error = state == vm::machine::VmState::Error;

                        if is_waiting || is_finished || is_error {
                            if let Some(timer) = session.execution_timer.take() {
                                session.accumulated_time += timer.elapsed();
                            }

                            // Set execution time for finished state
                            if is_finished {
                                execution_time_ms =
                                    Some(session.accumulated_time.as_secs_f64() * 1000.0);
                            }
                        }

                        let state_str = vm_state_to_string(state);

                        serde_wasm_bindgen::to_value(&VmStateInfo {
                            state: state_str.to_string(),
                            output: session.io.drain_output(),
                            execution_time_ms,
                        })
                        .unwrap()
                    }
                    Err(err) => {
                        // Stop timer on error
                        if let Some(timer) = session.execution_timer.take() {
                            session.accumulated_time += timer.elapsed();
                        }

                        // Get previous output and append error message with red color
                        let previous_output = session.io.drain_output();
                        let error_msg = format!(
                            "\x1b[31mRuntime Error: {}\n{:?}\x1b[0m",
                            err.message, err.kind
                        );
                        let combined_output = if previous_output.is_empty() {
                            error_msg
                        } else {
                            format!("{}\n{}", previous_output, error_msg)
                        };

                        serde_wasm_bindgen::to_value(&VmStateInfo {
                            state: "error".to_string(),
                            output: combined_output,
                            execution_time_ms: None,
                        })
                        .unwrap()
                    }
                }
            } else {
                serde_wasm_bindgen::to_value(&VmStateInfo {
                    state: "error".to_string(),
                    output: "No active program".to_string(),
                    execution_time_ms: None,
                })
                .unwrap()
            }
        })
    }

    /// Get current VM state and output
    #[wasm_bindgen]
    pub fn get_vm_state() -> JsValue {
        ACTIVE_SESSION.with(|s| {
            let mut session_opt = s.borrow_mut();
            if let Some(ref mut session) = *session_opt {
                let state = session.vm.get_state();
                let execution_time_ms = if state == vm::machine::VmState::Finished {
                    Some(session.accumulated_time.as_secs_f64() * 1000.0)
                } else {
                    None
                };

                serde_wasm_bindgen::to_value(&VmStateInfo {
                    state: vm_state_to_string(state).to_string(),
                    output: session.io.drain_output(),
                    execution_time_ms,
                })
                .unwrap()
            } else {
                serde_wasm_bindgen::to_value(&VmStateInfo {
                    state: "error".to_string(),
                    output: "No active program".to_string(),
                    execution_time_ms: None,
                })
                .unwrap()
            }
        })
    }

    /// Stop the running program
    #[wasm_bindgen]
    pub fn stop_program() {
        ACTIVE_SESSION.with(|s| {
            *s.borrow_mut() = None;
        });
    }

    // Helper function to convert VmState to string
    fn vm_state_to_string(state: vm::machine::VmState) -> &'static str {
        match state {
            vm::machine::VmState::Running => "running",
            vm::machine::VmState::WaitingForInput => "waiting_for_input",
            vm::machine::VmState::Finished => "finished",
            vm::machine::VmState::Error => "error",
        }
    }
}
