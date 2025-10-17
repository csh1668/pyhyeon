pub mod builtins;
pub mod interpreter;
pub mod lexer;
pub mod parser;
pub mod runtime_io;
pub mod semantic;
pub mod types;
pub mod vm;

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
    let mut reached_eof = false;
    let token_iter = std::iter::from_fn(move || {
        if reached_eof {
            return None;
        }
        let (t, span) = lexer.next_token_with_span();
        if t == lexer::token::Token::Eof {
            reached_eof = true;
            return None;
        }
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

pub fn run_interpreter(program: &[parser::ast::StmtS]) -> Result<(), Diagnostic> {
    let mut interp = interpreter::Interpreter::new();
    match interp.run(program) {
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
    use wasm_bindgen::prelude::*;

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

    /// Push a line into the VM input queue (to be consumed by input()).
    #[wasm_bindgen]
    pub fn push_input(_session: u32, line: &str) {
        // Simple single-VM model for now: keep a global VM later if session needed
        let _ = _session;
        // No-op placeholder; advanced interactive session will manage VM instance lifecycle.
        let _ = line;
    }
}
