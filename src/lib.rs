pub mod lexer;
pub mod parser;
pub mod semantic;
pub mod types;
pub mod interpreter;
pub mod vm;
pub mod builtins;

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
    let bytes = bincode::serialize(module).expect("serialize module");
    std::fs::write(path, bytes)
}

pub fn load_module(path: &str) -> std::io::Result<vm::bytecode::Module> {
    let bytes = std::fs::read(path)?;
    let module: vm::bytecode::Module = bincode::deserialize(&bytes).expect("deserialize module");
    Ok(module)
}


