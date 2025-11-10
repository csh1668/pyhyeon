use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r]+")]
#[logos(skip r"#[^\n]*")]
pub enum RawToken {
    // Keywords
    #[token("if")]
    If,
    #[token("elif")]
    Elif,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("def")]
    Def,
    #[token("return")]
    Return,
    #[token("and")]
    And,
    #[token("or")]
    Or,
    #[token("not")]
    Not,
    #[token("class")]
    Class,

    // Identifiers and literals
    #[token("None")]
    None,
    #[token("True", |_| true)]
    #[token("False", |_| false)]
    Bool(bool),
    #[regex(r"[0-9]+", lex_integer)]
    Int(i64),
    #[regex(r#""([^"\\]|\\.)*""#, lex_string)]
    #[regex(r#"'([^'\\]|\\.)*'"#, lex_string)]
    String(String),
    #[regex(r"[0-9]+\.[0-9]+", lex_float)]
    #[regex(r"[0-9]+\.[0-9]+[eE][+-]?[0-9]+", lex_float)]
    Float(f64),

    #[regex(r"[_\p{XID_Start}]\p{XID_Continue}*", lex_identifier)]
    Identifier(String),

    // Operators and punctuation
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("//")]
    SlashSlash,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("==")]
    EqualEqual,
    #[token("!=")]
    NotEqual,
    #[token("<")]
    Less,
    #[token("<=")]
    LessEqual,
    #[token(">")]
    Greater,
    #[token(">=")]
    GreaterEqual,
    #[token("=")]
    Equal,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,
    #[token(";")]
    Semicolon,
    #[token(".")]
    Dot,

    #[token("\n")]
    Newline,
}

fn lex_integer(lexer: &mut logos::Lexer<RawToken>) -> Option<i64> {
    let slice = lexer.slice();
    slice.parse::<i64>().ok()
}

fn lex_float(lexer: &mut logos::Lexer<RawToken>) -> Option<f64> {
    let slice = lexer.slice();
    slice.parse::<f64>().ok()
}

fn lex_string(lexer: &mut logos::Lexer<RawToken>) -> Option<String> {
    let slice = lexer.slice();
    let unquoted = &slice[1..slice.len() - 1];
    Some(process_string_escapes(unquoted))
}

fn lex_identifier(lexer: &mut logos::Lexer<RawToken>) -> Option<String> {
    let slice = lexer.slice();
    Some(slice.to_string())
}

fn process_string_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => result.push('\n'),
                        't' => result.push('\t'),
                        'r' => result.push('\r'),
                        '\\' => result.push('\\'),
                        '"' => result.push('"'),
                        '\'' => result.push('\''),
                        other => {
                            result.push('\\');
                            result.push(other);
                        }
                    }
                } else {
                    result.push('\\');
                }
            }
            other => result.push(other),
        }
    }
    result
}
