mod lexer;
mod parser;
mod semantic;

use ariadne::{Color, Label, Report, ReportKind, Source};
use chumsky::Parser;
use chumsky::input::{Input, Stream};
use chumsky::span::SimpleSpan;
use lexer::Lexer;

fn main() {
    let path = "./test.pyh";
    let src = std::fs::read_to_string(path).expect("Failed to read source file");

    let mut lexer = Lexer::new(src.as_str());
    let mut tokens = vec![];
    loop {
        let t = lexer.next_token();
        if t == lexer::token::Token::Eof {
            break;
        }
        tokens.push(t);
    }
    println!("Tokens: {:#?}", tokens);

    let mut lexer = Lexer::new(src.as_str());

    let mut reached_eof = false;
    let token_iter = std::iter::from_fn(move || {
        if reached_eof {
            return None;
        }
        let (t, span) = lexer.next_token_with_span();
        if t == lexer::token::Token::Eof {
            reached_eof = true;
            return None; // Do not include EOF token in the parser stream
        }
        Some((t, SimpleSpan::new(span.start, span.end)))
    });

    let eoi_span = parser::SimpleSpan::new(0, src.len());
    let token_stream = Stream::from_iter(token_iter).map(eoi_span, |(t, s)| (t, s));

    match parser::program_parser().parse(token_stream).into_result() {
        Ok(program) => {
            println!("Program: {:#?}", program);
            if let Err(e) = semantic::analyze(&program) {
                eprintln!("Semantic error: {}", e.message);
            }
        }
        Err(errors) => {
            for e in errors {
                Report::build(ReportKind::Error, ((), e.span().into_range()))
                    .with_config(ariadne::Config::new().with_index_type(ariadne::IndexType::Byte))
                    .with_code(3)
                    .with_message(e.reason().to_string())
                    .with_label(
                        Label::new(((), e.span().into_range()))
                            .with_message(e.reason().to_string())
                            .with_color(Color::Red),
                    )
                    .finish()
                    .eprint(Source::from(&src))
                    .unwrap();
            }
        }
    };
}
