pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod types;
pub mod interpreter;
pub mod vm;
pub mod builtins;
pub mod runtime_io;

use ariadne::{Color, Label, Report, ReportKind, Source};
use chumsky::Parser;
use chumsky::input::{Input, Stream};
use chumsky::span::SimpleSpan;

pub fn parse_source(path: &str, src: &str) -> Result<Vec<parser::ast::StmtS>, ()> {
    let mut lexer = lexer::Lexer::new(src);
    let mut reached_eof = false;
    let token_iter = std::iter::from_fn(move || {
        if reached_eof { return None; }
        let (t, span) = lexer.next_token_with_span();
        if t == lexer::token::Token::Eof { reached_eof = true; return None; }
        Some((t, SimpleSpan::new(span.start, span.end)))
    });
    let eoi_span = parser::SimpleSpan::new(src.len(), src.len());
    let token_stream = Stream::from_iter(token_iter).map(eoi_span, |(t, s)| (t, s));
    match parser::program_parser().parse(token_stream).into_result() {
        Ok(program) => Ok(program),
        Err(errors) => {
            let mut errors = errors;
            errors.sort_by(|x1, x2| {
                let x1 = (x1.span().start, x1.span().end);
                let x2 = (x2.span().start, x2.span().end);
                x1.cmp(&x2)
            });
            for e in errors {
                Report::build(ReportKind::Error, (path, e.span().into_range()))
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_code(3)
                    .with_message("Parsing failed")
                    .with_label(
                        Label::new((path, e.span().into_range()))
                            .with_message(e.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint((path, Source::from(src)))
                    .ok();
            }
            Err(())
        }
    }
}

pub fn analyze(program: &[parser::ast::StmtS], path: &str, src: &str) -> bool {
    match semantic::analyze(program) {
        Ok(_) => true,
        Err(e) => {
            Report::build(ReportKind::Error, (path, e.span.clone()))
                .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                .with_code(4)
                .with_message("Semantic Analyzing Failed")
                .with_label(
                    Label::new((path, e.span.clone()))
                        .with_message(e.message)
                        .with_color(Color::Red),
                )
                .finish()
                .eprint((path, Source::from(src)))
                .ok();
            false
        }
    }
}

pub fn run_interpreter(program: &[parser::ast::StmtS], path: &str, src: &str) {
    let mut interp = interpreter::Interpreter::new();
    if let Err(e) = interp.run(program) {
        Report::build(ReportKind::Error, (path, e.span.clone()))
            .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
            .with_code(5)
            .with_message("Runtime Error")
            .with_label(
                Label::new((path, e.span.clone()))
                    .with_message(e.message)
                    .with_color(Color::Red),
            )
            .finish()
            .eprint((path, Source::from(src)))
            .ok();
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
    let bytes = bincode::serde::encode_to_vec(module, cfg)
        .expect("serialize module");
    std::fs::write(path, bytes)
}

pub fn load_module(path: &str) -> std::io::Result<vm::bytecode::Module> {
    let bytes = std::fs::read(path)?;
    let cfg = bincode::config::standard();
    let (module, _consumed): (vm::bytecode::Module, usize) =
        bincode::serde::decode_from_slice(&bytes, cfg)
            .expect("deserialize module");
    Ok(module)
}

// ===== wasm_bindgen exports for web playground =====
#[cfg(target_arch = "wasm32")]
mod wasm_api {
    use super::*;
    use wasm_bindgen::prelude::*;
    use serde::Serialize;

    #[derive(Serialize)]
    pub struct WasmDiagnostic {
        pub message: String,
        pub start_line: u32,
        pub start_char: u32,
        pub end_line: u32,
        pub end_char: u32,
        pub severity: u8,
    }

    fn byte_to_lc(src: &str, byte_idx: usize) -> (u32, u32) {
        let prefix = &src[..byte_idx.min(src.len())];
        let line = prefix.bytes().filter(|&b| b == b'\n').count() as u32;
        let line_start = prefix.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = src[line_start..byte_idx.min(src.len())].encode_utf16().count() as u32;
        (line, col)
    }

    #[wasm_bindgen]
    pub fn analyze(src: &str) -> JsValue {
        // parse
        let program = match super::parse_source("<mem>", src) {
            Ok(p) => p,
            Err(_) => return serde_wasm_bindgen::to_value(&Vec::<WasmDiagnostic>::new()).unwrap(),
        };
        // semantic
        if let Err(e) = super::semantic::analyze(&program) {
            let (sl, sc) = byte_to_lc(src, e.span.start);
            let (el, ec) = byte_to_lc(src, e.span.end);
            let diag = WasmDiagnostic {
                message: e.message,
                start_line: sl,
                start_char: sc,
                end_line: el,
                end_char: ec,
                severity: 1,
            };
            return serde_wasm_bindgen::to_value(&vec![diag]).unwrap();
        }
        serde_wasm_bindgen::to_value(&Vec::<WasmDiagnostic>::new()).unwrap()
    }

    fn format_error_report(path: &str, src: &str, span: std::ops::Range<usize>, message: String, kind: &str) -> String {
        use std::fmt::Write;
        let mut output = String::new();
        
        // Calculate line/column information
        let (start_line, start_col) = byte_to_lc(src, span.start);
        let (end_line, end_col) = byte_to_lc(src, span.end);
        
        // Write header
        writeln!(&mut output, "Error: {}", kind).ok();
        writeln!(&mut output, "  ╭─[{}:{}:{}]", path, start_line + 1, start_col + 1).ok();
        
        // Extract and display relevant lines
        let lines: Vec<&str> = src.lines().collect();
        let start_display = start_line.saturating_sub(1) as usize;
        let end_display = ((end_line + 2) as usize).min(lines.len());
        
        for (idx, line) in lines[start_display..end_display].iter().enumerate() {
            let line_num = start_display + idx;
            let display_num = line_num + 1;
            
            if line_num == start_line as usize {
                writeln!(&mut output, "{:>3} │ {}", display_num, line).ok();
                // Add error marker
                let marker_start = if line_num == start_line as usize { start_col as usize } else { 0 };
                let marker_end = if line_num == end_line as usize { 
                    end_col as usize 
                } else { 
                    line.len() 
                };
                let marker_len = (marker_end - marker_start).max(1);
                write!(&mut output, "    · {}", " ".repeat(marker_start)).ok();
                writeln!(&mut output, "{} {}", "^".repeat(marker_len), message).ok();
            } else {
                writeln!(&mut output, "{:>3} │ {}", display_num, line).ok();
            }
        }
        
        writeln!(&mut output, "  ╰────").ok();
        output
    }

    /// Compile and execute via VM, capturing output; return it as a string.
    #[wasm_bindgen]
    pub fn run(src: &str) -> String {
        let start_time = instant::Instant::now();
        let src = if src.ends_with('\n') { src.to_string() } else { format!("{}\n", src) };
        
        // Parse
        let program = match super::parse_source("<mem>", &src) {
            Ok(p) => p,
            Err(_) => {
                // Try to get detailed parse error
                let mut lexer = super::lexer::Lexer::new(&src);
                let mut reached_eof = false;
                let token_iter = std::iter::from_fn(move || {
                    if reached_eof { return None; }
                    let (t, span) = lexer.next_token_with_span();
                    if t == super::lexer::token::Token::Eof { reached_eof = true; return None; }
                    Some((t, SimpleSpan::new(span.start, span.end)))
                });
                let eoi_span = SimpleSpan::new(src.len(), src.len());
                let token_stream = Stream::from_iter(token_iter).map(eoi_span, |(t, s)| (t, s));
                
                if let Err(errors) = super::parser::program_parser().parse(token_stream).into_result() {
                    if let Some(first_error) = errors.first() {
                        let span = first_error.span().into_range();
                        let message = first_error.reason().to_string();
                        return format_error_report("<mem>", &src, span, message, "Parsing Failed");
                    }
                }
                return "Parse error".into();
            }
        };
        
        // Semantic analysis
        if let Err(e) = super::semantic::analyze(&program) {
            return format_error_report("<mem>", &src, e.span, e.message, "Semantic Error");
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
                    format!("\n────────────────────────────────────\nExecution time: {:.3}ms\nTotal time: {:.3}ms", 
                        exec_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0)
                } else {
                    format!("{}\n────────────────────────────────────\nExecution time: {:.3}ms\nTotal time: {:.3}ms", 
                        output.trim_end(),
                        exec_time.as_secs_f64() * 1000.0,
                        total_time.as_secs_f64() * 1000.0)
                }
            },
            Err(err) => format!("Runtime Error: {}\n{:?}", err.message, err.kind),
        };
        result
    }

    /// Push a line into the VM input queue (to be consumed by input()).
    #[wasm_bindgen]
    pub fn push_input(_session: u32, line: &str) {
        // Simple single-VM model for now: keep a global VM later if session needed
        let _ = _session;
        // No-op placeholder; advanced interactive session will manage VM instance lifecycle.
        let _ = line;
    }
}


