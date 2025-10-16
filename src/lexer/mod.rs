mod raw_token;
pub mod token;

use logos::{Lexer as LogosLexer, Logos};
use raw_token::RawToken;
use std::collections::VecDeque;
use std::ops::Range;
pub(crate) use token::Token;

pub struct Lexer<'source> {
    inner: LogosLexer<'source, RawToken>,
    token_queue: VecDeque<(Token, Range<usize>)>,
    indent_stack: Vec<usize>,
    at_start_of_line: bool,
}

impl<'source> Lexer<'source> {
    pub fn new(source: &'source str) -> Self {
        Lexer {
            inner: RawToken::lexer(source),
            token_queue: VecDeque::new(),
            indent_stack: vec![0],
            at_start_of_line: true,
        }
    }

    pub fn next_token(&mut self) -> Token {
        let (tok, _) = self.next_token_with_span();
        tok
    }

    pub fn next_token_with_span(&mut self) -> (Token, Range<usize>) {
        if let Some((token, span)) = self.token_queue.pop_front() {
            return (token, span);
        }

        if self.at_start_of_line {
            self.handle_indentation();
            if let Some((token, span)) = self.token_queue.pop_front() {
                return (token, span);
            }
        }

        match self.inner.next() {
            Some(Ok(raw_token)) => {
                let span = self.inner.span();
                let token = Self::convert_token(raw_token);
                if token == Token::Newline {
                    self.at_start_of_line = true;
                }
                (token, span)
            }
            Some(Err(_)) => {
                let span = self.inner.span();
                let error_msg = format!("Invalid token '{}'", self.inner.slice());
                (Token::Error(error_msg, span.clone()), span)
            }
            None => {
                while self.indent_stack.len() > 1 {
                    self.indent_stack.pop();
                    // Use the current cursor position as zero-length span for dedent
                    let pos = self.inner.span().end;
                    self.token_queue.push_back((Token::Dedent, pos..pos));
                }
                if let Some((tok, span)) = self.token_queue.pop_front() {
                    (tok, span)
                } else {
                    let pos = self.inner.span().end;
                    (Token::Eof, pos..pos)
                }
            }
        }
    }

    fn handle_indentation(&mut self) {
        assert!(
            self.at_start_of_line,
            "handle_indentation should be called at the start of a line"
        );
        let line_start = self.inner.span().end; // last token was Newline
        let remainder = self.inner.remainder();

        let mut current_indent = 0;
        for ch in remainder.chars() {
            match ch {
                ' ' => current_indent += 1,
                '\t' => {
                    // tab is not allowed
                    let tab_span = (line_start + current_indent)..(line_start + current_indent + 1);
                    self.token_queue.push_back((
                        Token::Error(
                            "Tabs are not allowed for indentation.".to_string(),
                            tab_span.clone(),
                        ),
                        tab_span,
                    ));
                }
                _ => break,
            }
        }

        self.inner.bump(current_indent);
        let indent_span = line_start..(line_start + current_indent);
        let next_char = remainder.chars().nth(current_indent);

        if let Some(ch) = next_char {
            let ignore = ['\n', '\r', '#'];
            if ignore.contains(&ch) {
                self.at_start_of_line = true;
                return;
            }
        }

        self.at_start_of_line = false;

        let last_indent = *self.indent_stack.last().unwrap_or(&0);
        if current_indent == last_indent {
            // Same level, do nothing
        } else if current_indent > last_indent {
            if current_indent != last_indent + 2 {
                self.token_queue.push_back((
                    Token::Error(
                        format!(
                            "Invalid indentation: expected {} spaces, but got {}.",
                            last_indent + 2,
                            current_indent
                        ),
                        indent_span.clone(),
                    ),
                    indent_span,
                ));
            } else {
                self.indent_stack.push(current_indent);
                self.token_queue.push_back((Token::Indent, indent_span));
            }
        } else {
            // current_indent < last_indent
            while current_indent < *self.indent_stack.last().unwrap_or(&0) {
                self.indent_stack.pop();
                self.token_queue
                    .push_back((Token::Dedent, indent_span.clone()));
            }
            if current_indent != *self.indent_stack.last().unwrap_or(&0) {
                self.token_queue.push_back((
                    Token::Error("Invalid dedentation.".to_string(), indent_span.clone()),
                    indent_span,
                ));
            }
        }
    }

    fn convert_token(raw: RawToken) -> Token {
        match raw {
            RawToken::None => Token::None,
            RawToken::If => Token::If,
            RawToken::Elif => Token::Elif,
            RawToken::Else => Token::Else,
            RawToken::While => Token::While,
            RawToken::Def => Token::Def,
            RawToken::Return => Token::Return,
            RawToken::And => Token::And,
            RawToken::Or => Token::Or,
            RawToken::Not => Token::Not,
            RawToken::Bool(b) => Token::Bool(b),
            RawToken::Int(i) => Token::Int(i),
            RawToken::Identifier(name) => Token::Identifier(name),
            RawToken::Plus => Token::Plus,
            RawToken::Minus => Token::Minus,
            RawToken::Star => Token::Star,
            RawToken::SlashSlash => Token::SlashSlash,
            RawToken::Percent => Token::Percent,
            RawToken::EqualEqual => Token::EqualEqual,
            RawToken::NotEqual => Token::NotEqual,
            RawToken::Less => Token::Less,
            RawToken::LessEqual => Token::LessEqual,
            RawToken::Greater => Token::Greater,
            RawToken::GreaterEqual => Token::GreaterEqual,
            RawToken::Equal => Token::Equal,
            RawToken::LParen => Token::LParen,
            RawToken::RParen => Token::RParen,
            RawToken::Colon => Token::Colon,
            RawToken::Comma => Token::Comma,
            RawToken::Semicolon => Token::Semicolon,
            RawToken::Newline => Token::Newline,
            _ => unreachable!("unhandled RawToken variant"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factorial() {
        let source = "\
def factorial(n):
  if n == 0:
    return 1
  else:
    return n * factorial(n - 1)
";
        let mut lexer = Lexer::new(source);
        let expected_tokens = vec![
            Token::Def,
            Token::Identifier("factorial".to_string()),
            Token::LParen,
            Token::Identifier("n".to_string()),
            Token::RParen,
            Token::Colon,
            Token::Newline,
            Token::Indent,
            Token::If,
            Token::Identifier("n".to_string()),
            Token::EqualEqual,
            Token::Int(0),
            Token::Colon,
            Token::Newline,
            Token::Indent,
            Token::Return,
            Token::Int(1),
            Token::Newline,
            Token::Dedent,
            Token::Else,
            Token::Colon,
            Token::Newline,
            Token::Indent,
            Token::Return,
            Token::Identifier("n".to_string()),
            Token::Star,
            Token::Identifier("factorial".to_string()),
            Token::LParen,
            Token::Identifier("n".to_string()),
            Token::Minus,
            Token::Int(1),
            Token::RParen,
            Token::Newline,
            Token::Dedent,
            Token::Dedent,
            Token::Eof,
        ];
        for expected in expected_tokens {
            let token = lexer.next_token();
            assert_eq!(token, expected);
        }
    }

    #[test]
    fn two_functions() {
        let source = "\
def add(a, b):
  return a + b
def sub(a, b):
  return a - b
";
        let mut lexer = Lexer::new(source);
        let expected_tokens = vec![
            Token::Def,
            Token::Identifier("add".to_string()),
            Token::LParen,
            Token::Identifier("a".to_string()),
            Token::Comma,
            Token::Identifier("b".to_string()),
            Token::RParen,
            Token::Colon,
            Token::Newline,
            Token::Indent,
            Token::Return,
            Token::Identifier("a".to_string()),
            Token::Plus,
            Token::Identifier("b".to_string()),
            Token::Newline,
            Token::Dedent,
            Token::Def,
            Token::Identifier("sub".to_string()),
            Token::LParen,
            Token::Identifier("a".to_string()),
            Token::Comma,
            Token::Identifier("b".to_string()),
            Token::RParen,
            Token::Colon,
            Token::Newline,
            Token::Indent,
            Token::Return,
            Token::Identifier("a".to_string()),
            Token::Minus,
            Token::Identifier("b".to_string()),
            Token::Newline,
            Token::Dedent,
            Token::Eof,
        ];
        for expected in expected_tokens {
            let token = lexer.next_token();
            assert_eq!(token, expected);
        }
    }
}
