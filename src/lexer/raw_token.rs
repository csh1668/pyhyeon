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

    // Identifiers and literals
    #[token("True", |_| true)]
    #[token("False", |_| false)]
    Bool(bool),
    #[regex(r"[0-9]+", lex_integer)]
    Int(i64),
    #[regex(r"\p{XID_Start}\p{XID_Continue}*", lex_identifier)]
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
    #[token(":")]
    Colon,
    #[token(",")]
    Comma,

    #[token("\n")]
    Newline,
}

fn lex_integer(lexer: &mut logos::Lexer<RawToken>) -> Option<i64> {
    let slice = lexer.slice();
    slice.parse::<i64>().ok()
}

fn lex_identifier(lexer: &mut logos::Lexer<RawToken>) -> Option<String> {
    let slice = lexer.slice();
    Some(slice.to_string())
}
