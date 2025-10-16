use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    If,
    Elif,
    Else,
    While,
    Def,
    Return,
    And,
    Or,
    Not,
    // Identifiers and literals
    None,
    Bool(bool),
    Int(i64),
    Identifier(String),
    // Operators and punctuation
    Plus,
    Minus,
    Star,
    SlashSlash,
    Percent,
    EqualEqual,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Equal,
    LParen,
    RParen,
    Colon,
    Comma,
    Semicolon,
    // Special tokens
    Indent,
    Dedent,
    Newline,
    Eof,

    Error(String, Range<usize>),
}

impl Display for Token {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            // Keywords
            Token::If => write!(f, "if"),
            Token::Elif => write!(f, "elif"),
            Token::Else => write!(f, "else"),
            Token::While => write!(f, "while"),
            Token::Def => write!(f, "def"),
            Token::Return => write!(f, "return"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Not => write!(f, "not"),

            // Identifiers and literals
            Token::None => write!(f, "None"),
            Token::Bool(true) => write!(f, "True"),
            Token::Bool(false) => write!(f, "False"),
            Token::Int(i) => write!(f, "{}", i),
            Token::Identifier(name) => write!(f, "{}", name),

            // Operators and punctuation
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::SlashSlash => write!(f, "//"),
            Token::Percent => write!(f, "%"),
            Token::EqualEqual => write!(f, "=="),
            Token::NotEqual => write!(f, "!="),
            Token::Less => write!(f, "<"),
            Token::LessEqual => write!(f, "<="),
            Token::Greater => write!(f, ">"),
            Token::GreaterEqual => write!(f, ">="),
            Token::Equal => write!(f, "="),
            Token::LParen => write!(f, "("),
            Token::RParen => write!(f, ")"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Semicolon => write!(f, ";"),

            // Special tokens
            Token::Indent => write!(f, "<INDENT>"),
            Token::Dedent => write!(f, "<DEDENT>"),
            Token::Newline => write!(f, "\\n"),
            Token::Eof => write!(f, "<EOF>"),

            Token::Error(msg, _) => write!(f, "<ERROR: {}>", msg),
        }
    }
}
