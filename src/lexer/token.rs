use std::fmt::{Display, Formatter, Result as FmtResult};
use std::ops::Range;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    If,
    Elif,
    Else,
    While,
    For,
    In,
    Def,
    Return,
    And,
    Or,
    Not,
    Class,
    Break,
    Continue,
    Pass,
    // Identifiers and literals
    None,
    Bool(bool),
    Int(i64),
    String(String),
    Identifier(String),
    Float(f64),
    // Operators and punctuation
    Plus,
    Minus,
    Star,
    SlashSlash,
    Slash,
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
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Semicolon,
    Dot,
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
            Token::For => write!(f, "for"),
            Token::In => write!(f, "in"),
            Token::Def => write!(f, "def"),
            Token::Return => write!(f, "return"),
            Token::And => write!(f, "and"),
            Token::Or => write!(f, "or"),
            Token::Not => write!(f, "not"),
            Token::Class => write!(f, "class"),
            Token::Break => write!(f, "break"),
            Token::Continue => write!(f, "continue"),
            Token::Pass => write!(f, "pass"),

            // Identifiers and literals
            Token::None => write!(f, "None"),
            Token::Bool(true) => write!(f, "True"),
            Token::Bool(false) => write!(f, "False"),
            Token::Int(i) => write!(f, "{}", i),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Identifier(name) => write!(f, "{}", name),
            Token::Float(ff) => write!(f, "{}", ff),

            // Operators and punctuation
            Token::Plus => write!(f, "+"),
            Token::Minus => write!(f, "-"),
            Token::Star => write!(f, "*"),
            Token::SlashSlash => write!(f, "//"),
            Token::Slash => write!(f, "/"),
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
            Token::LBracket => write!(f, "["),
            Token::RBracket => write!(f, "]"),
            Token::LBrace => write!(f, "{{"),
            Token::RBrace => write!(f, "}}"),
            Token::Colon => write!(f, ":"),
            Token::Comma => write!(f, ","),
            Token::Semicolon => write!(f, ";"),
            Token::Dot => write!(f, "."),

            // Special tokens
            Token::Indent => write!(f, "<INDENT>"),
            Token::Dedent => write!(f, "<DEDENT>"),
            Token::Newline => write!(f, "\\n"),
            Token::Eof => write!(f, "<EOF>"),

            Token::Error(msg, _) => write!(f, "<ERROR: {}>", msg),
        }
    }
}
