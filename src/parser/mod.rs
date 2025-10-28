pub(crate) mod ast;

use crate::lexer::Token;
use crate::types::Span;
use ast::*;
use chumsky::Parser;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub use chumsky::span::SimpleSpan;

type RichTokenError<'a> = Rich<'a, Token>;

#[derive(Debug, Clone)]
enum PostfixOp {
    Attr(String),
    Call(Vec<ExprS>),
}

pub fn expr_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExprS, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    recursive(|expr| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        // Primary: literals, variables, parenthesized expressions
        let primary = choice((
            select! {
                Token::Int(i) => Expr::Literal(Literal::Int(i)),
                Token::Bool(b) => Expr::Literal(Literal::Bool(b)),
                Token::String(s) => Expr::Literal(Literal::String(s)),
                Token::None => Expr::Literal(Literal::None),
            }
            .labelled("literal"),
            ident.map(Expr::Variable),
            expr.clone()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .map(|e: ExprS| e.0), // take inner expr node
        ))
        .map_with(|node: Expr, e| {
            let s: I::Span = e.span();
            (node, s.into_range())
        });

        // Postfix: handles . and () chaining
        // Example: obj.attr, obj.method(), obj.method().attr
        let postfix_op = choice((
            // .attr (attribute access)
            just(Token::Dot)
                .ignore_then(ident.clone())
                .map(|attr| PostfixOp::Attr(attr)),
            // (args) (function/method call)
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .map(|args| PostfixOp::Call(args)),
        ));

        let atom = primary.foldl(postfix_op.repeated(), |base: ExprS, op: PostfixOp| {
            let start = base.1.start;
            match op {
                PostfixOp::Attr(attr) => {
                    let end = start + attr.len(); // rough estimate
                    (
                        Expr::Attribute {
                            object: Box::new(base),
                            attr,
                        },
                        start..end,
                    )
                }
                PostfixOp::Call(args) => {
                    let end = start + 10; // rough estimate
                    (
                        Expr::Call {
                            func_name: Box::new(base),
                            args,
                        },
                        start..end,
                    )
                }
            }
        });

        // Capture unary operator with its span
        let op_unary = choice((
            just(Token::Not).to(UnaryOp::Not),
            just(Token::Minus).to(UnaryOp::Negate),
            just(Token::Plus).to(UnaryOp::Pos),
        ))
        .map_with(|op, e| {
            let s: I::Span = e.span();
            (op, s.into_range())
        });

        let unary =
            op_unary
                .repeated()
                .foldr(atom, |(op, op_span): (UnaryOp, Span), right: ExprS| {
                    let span = op_span.start..right.1.end;
                    (
                        Expr::Unary {
                            op,
                            expr: Box::new(right),
                        },
                        span,
                    )
                });

        let op = |t| just(t).ignored();
        let product = unary.clone().foldl(
            choice((
                op(Token::Star).to(BinaryOp::Multiply),
                op(Token::SlashSlash).to(BinaryOp::FloorDivide),
                op(Token::Percent).to(BinaryOp::Modulo),
            ))
            .then(unary)
            .repeated(),
            |left: ExprS, (op, right): (BinaryOp, ExprS)| {
                let span = left.1.start..right.1.end;
                (
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                )
            },
        );
        let sum = product.clone().foldl(
            choice((
                op(Token::Plus).to(BinaryOp::Add),
                op(Token::Minus).to(BinaryOp::Subtract),
            ))
            .then(product)
            .repeated(),
            |left: ExprS, (op, right): (BinaryOp, ExprS)| {
                let span = left.1.start..right.1.end;
                (
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                )
            },
        );
        let comparison = sum.clone().foldl(
            choice((
                op(Token::Less).to(BinaryOp::Less),
                op(Token::LessEqual).to(BinaryOp::LessEqual),
                op(Token::Greater).to(BinaryOp::Greater),
                op(Token::GreaterEqual).to(BinaryOp::GreaterEqual),
                op(Token::EqualEqual).to(BinaryOp::Equal),
                op(Token::NotEqual).to(BinaryOp::NotEqual),
            ))
            .then(sum)
            .repeated(),
            |left: ExprS, (op, right): (BinaryOp, ExprS)| {
                let span = left.1.start..right.1.end;
                (
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                )
            },
        );
        let and_expr = comparison.clone().foldl(
            op(Token::And).to(BinaryOp::And).then(comparison).repeated(),
            |left: ExprS, (op, right): (BinaryOp, ExprS)| {
                let span = left.1.start..right.1.end;
                (
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                )
            },
        );
        let or_expr = and_expr.clone().foldl(
            op(Token::Or).to(BinaryOp::Or).then(and_expr).repeated(),
            |left: ExprS, (op, right): (BinaryOp, ExprS)| {
                let span = left.1.start..right.1.end;
                (
                    Expr::Binary {
                        op,
                        left: Box::new(left),
                        right: Box::new(right),
                    },
                    span,
                )
            },
        );

        or_expr.labelled("expression")
    })
    .boxed()
}

pub fn stmt_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<StmtS>, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    let expr = expr_parser().boxed();

    recursive(|stmt| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        // Line end (either newline or end of input)
        let line_end = just(Token::Newline).ignored().or(end().ignored());

        // Simple statement kinds (no line ending consumption here)
        let return_stmt = just(Token::Return)
            .ignore_then(expr.clone())
            .map(Stmt::Return)
            .labelled("return statement");

        let assign_stmt = expr
            .clone()
            .then_ignore(just(Token::Equal))
            .then(expr.clone())
            .map_with(|(target, value), e| {
                let s: I::Span = e.span();
                Stmt::Assign {
                    target: (target.0, s.into_range()),
                    value,
                }
            })
            .labelled("assignment");

        let expr_stmt = expr
            .clone()
            .map(Stmt::Expr)
            .labelled("expression statement");

        // A line of one or more simple statements separated by ';' with optional trailing ';'
        let simple_stmt = choice((return_stmt.clone(), assign_stmt.clone(), expr_stmt.clone()))
            .map_with(|node: Stmt, e| {
                let s: I::Span = e.span();
                (node, s.into_range())
            });

        let simple_stmts_line = simple_stmt
            .separated_by(just(Token::Semicolon))
            .allow_trailing()
            .at_least(1)
            .collect::<Vec<StmtS>>()
            .then_ignore(line_end.clone())
            .labelled("simple statements");

        // Helper: parse a block after a ':'
        // Support both indented blocks and inline simple_stmts on the same line
        let indented_block = stmt
            .clone()
            .repeated()
            .collect::<Vec<Vec<StmtS>>>()
            .map(|lines| lines.into_iter().flatten().collect::<Vec<StmtS>>())
            .delimited_by(
                just(Token::Newline)
                    .ignore_then(just(Token::Newline).ignored().repeated())
                    .ignore_then(just(Token::Indent)),
                just(Token::Dedent),
            );

        let block =
            just(Token::Colon).ignore_then(choice((indented_block, simple_stmts_line.clone())));

        let def_stmt = just(Token::Def)
            .ignore_then(ident)
            .then(
                ident
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then(block.clone())
            .map(
                |((name, params), body): ((String, Vec<String>), Vec<StmtS>)| Stmt::Def {
                    name,
                    params,
                    body,
                },
            )
            .labelled("def statement");

        let if_stmt = just(Token::If)
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(
                just(Token::Elif)
                    .ignore_then(expr.clone())
                    .then(block.clone())
                    .repeated()
                    .collect::<Vec<(ExprS, Vec<StmtS>)>>(),
            )
            .then(just(Token::Else).ignore_then(block.clone()).or_not())
            .map(
                |(((condition, then_block), elif_blocks), else_block)| Stmt::If {
                    condition,
                    then_block,
                    elif_blocks,
                    else_block,
                },
            )
            .labelled("if statement");

        let while_stmt = just(Token::While)
            .ignore_then(expr.clone())
            .then(block.clone())
            .map(|(condition, body)| Stmt::While { condition, body })
            .labelled("while statement");

        // Class statement
        let method_def = just(Token::Def)
            .ignore_then(ident.clone())
            .then(
                ident
                    .clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then(block.clone())
            .map(|((name, params), body)| MethodDef { name, params, body });

        let class_stmt = just(Token::Class)
            .ignore_then(ident.clone())
            .then_ignore(just(Token::Colon))
            .then(
                just(Token::Newline)
                    .ignore_then(just(Token::Newline).ignored().repeated())
                    .ignore_then(just(Token::Indent))
                    .ignore_then(
                        method_def
                            .map_with(|m, e| {
                                let s: I::Span = e.span();
                                (m, s.into_range())
                            })
                            .repeated()
                            .at_least(1)
                            .collect::<Vec<(MethodDef, Span)>>(),
                    )
                    .then_ignore(just(Token::Dedent)),
            )
            .map(|(name, methods)| {
                let methods = methods.into_iter().map(|(m, _)| m).collect();
                Stmt::Class {
                    name,
                    methods,
                    attributes: vec![], // v1에서는 클래스 속성 미지원
                }
            })
            .labelled("class statement");

        // Compound statements occupy the whole logical line
        let compound_stmt_line = choice((class_stmt, def_stmt, if_stmt, while_stmt))
            .map_with(|node: Stmt, e| {
                let s: I::Span = e.span();
                (node, s.into_range())
            })
            .map(|s| vec![s]);

        // Allow empty lines between statements
        let blank_lines = just(Token::Newline).ignored().repeated();

        choice((compound_stmt_line, simple_stmts_line))
            .padded_by(blank_lines)
            .recover_with(skip_then_retry_until(
                any().ignored(),
                just(Token::Newline)
                    .ignored()
                    .or(just(Token::Dedent).ignored())
                    .or(end().ignored()),
            ))
    })
    .boxed()
}

pub fn program_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<StmtS>, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    let line = stmt_parser().boxed();
    let blanks = just(Token::Newline).ignored().repeated();

    blanks
        .clone()
        .ignore_then(line.clone().repeated().collect::<Vec<Vec<StmtS>>>())
        .map(|lines| lines.into_iter().flatten().collect::<Vec<StmtS>>())
        .then_ignore(blanks)
        .then_ignore(end())
        .boxed()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn tokenize(source: &str) -> Vec<(Token, SimpleSpan)> {
        let mut lexer = Lexer::new(source);
        let mut tokens = Vec::new();
        loop {
            let (token, span) = lexer.next_token_with_span();
            if token == Token::Eof {
                // Don't include Eof token, use stream end instead
                break;
            }
            tokens.push((token, SimpleSpan::from(span)));
        }
        tokens
    }

    fn parse_expr(source: &str) -> Result<ExprS, Vec<RichTokenError<'_>>> {
        let tokens = tokenize(source);
        let eoi_span = SimpleSpan::new(source.len(), source.len());
        let stream =
            chumsky::input::Stream::from_iter(tokens.into_iter()).map(eoi_span, |(t, s)| (t, s));
        expr_parser().parse(stream).into_result()
    }

    fn parse_program(source: &str) -> Result<Vec<StmtS>, Vec<RichTokenError<'_>>> {
        let tokens = tokenize(source);
        let eoi_span = SimpleSpan::new(source.len(), source.len());
        let stream =
            chumsky::input::Stream::from_iter(tokens.into_iter()).map(eoi_span, |(t, s)| (t, s));
        program_parser().parse(stream).into_result()
    }

    // ========== 표현식 파싱 테스트 ==========

    #[test]
    fn test_parse_literal_int() {
        let result = parse_expr("42");
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let expr = result.unwrap();
        assert!(matches!(expr.0, Expr::Literal(Literal::Int(42))));
    }

    #[test]
    fn test_parse_literal_bool() {
        let result = parse_expr("True");
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().0,
            Expr::Literal(Literal::Bool(true))
        ));

        let result = parse_expr("False");
        assert!(result.is_ok());
        assert!(matches!(
            result.unwrap().0,
            Expr::Literal(Literal::Bool(false))
        ));
    }

    #[test]
    fn test_parse_literal_none() {
        let result = parse_expr("None");
        assert!(result.is_ok());
        assert!(matches!(result.unwrap().0, Expr::Literal(Literal::None)));
    }

    #[test]
    fn test_parse_variable() {
        let result = parse_expr("x");
        assert!(result.is_ok());
        if let Expr::Variable(name) = result.unwrap().0 {
            assert_eq!(name, "x");
        } else {
            panic!("Expected variable");
        }
    }

    #[test]
    fn test_parse_binary_add() {
        let result = parse_expr("1 + 2");
        assert!(result.is_ok());
        let expr = result.unwrap();
        if let Expr::Binary { op, left, right } = expr.0 {
            assert!(matches!(op, BinaryOp::Add));
            assert!(matches!(left.0, Expr::Literal(Literal::Int(1))));
            assert!(matches!(right.0, Expr::Literal(Literal::Int(2))));
        } else {
            panic!("Expected binary expression");
        }
    }

    #[test]
    fn test_parse_binary_precedence() {
        // 1 + 2 * 3 should parse as 1 + (2 * 3)
        let result = parse_expr("1 + 2 * 3");
        assert!(result.is_ok());
        let expr = result.unwrap();
        if let Expr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } = expr.0
        {
            assert!(matches!(left.0, Expr::Literal(Literal::Int(1))));
            if let Expr::Binary {
                op: BinaryOp::Multiply,
                ..
            } = right.0
            {
                // correct precedence
            } else {
                panic!("Expected multiplication on right");
            }
        } else {
            panic!("Expected addition at top level");
        }
    }

    #[test]
    fn test_parse_unary_negate() {
        let result = parse_expr("-42");
        assert!(result.is_ok());
        if let Expr::Unary { op, expr } = result.unwrap().0 {
            assert!(matches!(op, UnaryOp::Negate));
            assert!(matches!(expr.0, Expr::Literal(Literal::Int(42))));
        } else {
            panic!("Expected unary negate");
        }
    }

    #[test]
    fn test_parse_unary_not() {
        let result = parse_expr("not True");
        assert!(result.is_ok());
        if let Expr::Unary { op, expr } = result.unwrap().0 {
            assert!(matches!(op, UnaryOp::Not));
            assert!(matches!(expr.0, Expr::Literal(Literal::Bool(true))));
        } else {
            panic!("Expected unary not");
        }
    }

    #[test]
    fn test_parse_call_no_args() {
        let result = parse_expr("foo()");
        assert!(result.is_ok());
        if let Expr::Call { func_name, args } = result.unwrap().0 {
            assert!(matches!(func_name.0, Expr::Variable(_)));
            assert_eq!(args.len(), 0);
        } else {
            panic!("Expected call");
        }
    }

    #[test]
    fn test_parse_call_with_args() {
        let result = parse_expr("add(1, 2)");
        assert!(result.is_ok());
        if let Expr::Call { func_name, args } = result.unwrap().0 {
            if let Expr::Variable(name) = &func_name.0 {
                assert_eq!(name, "add");
            } else {
                panic!("Expected variable");
            }
            assert_eq!(args.len(), 2);
            assert!(matches!(args[0].0, Expr::Literal(Literal::Int(1))));
            assert!(matches!(args[1].0, Expr::Literal(Literal::Int(2))));
        } else {
            panic!("Expected call");
        }
    }

    #[test]
    fn test_parse_comparison() {
        let result = parse_expr("x < 10");
        assert!(result.is_ok());
        if let Expr::Binary { op, .. } = result.unwrap().0 {
            assert!(matches!(op, BinaryOp::Less));
        } else {
            panic!("Expected comparison");
        }
    }

    #[test]
    fn test_parse_logical_and() {
        let result = parse_expr("True and False");
        assert!(result.is_ok());
        if let Expr::Binary { op, .. } = result.unwrap().0 {
            assert!(matches!(op, BinaryOp::And));
        } else {
            panic!("Expected logical and");
        }
    }

    #[test]
    fn test_parse_logical_or() {
        let result = parse_expr("True or False");
        assert!(result.is_ok());
        if let Expr::Binary { op, .. } = result.unwrap().0 {
            assert!(matches!(op, BinaryOp::Or));
        } else {
            panic!("Expected logical or");
        }
    }

    #[test]
    fn test_parse_parenthesized() {
        let result = parse_expr("(1 + 2) * 3");
        assert!(result.is_ok());
        // Should parse as (1+2)*3, not 1+(2*3)
        if let Expr::Binary {
            op: BinaryOp::Multiply,
            left,
            right,
        } = result.unwrap().0
        {
            assert!(matches!(
                left.0,
                Expr::Binary {
                    op: BinaryOp::Add,
                    ..
                }
            ));
            assert!(matches!(right.0, Expr::Literal(Literal::Int(3))));
        } else {
            panic!("Expected multiplication at top level");
        }
    }

    // ========== 문장 파싱 테스트 ==========

    #[test]
    fn test_parse_assign() {
        let source = "x = 42\n";
        let result = parse_program(source);
        assert!(result.is_ok(), "Failed to parse: {:?}", result.err());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Assign { target, value } = &stmts[0].0 {
            assert!(matches!(target.0, Expr::Variable(_)));
            if let Expr::Variable(name) = &target.0 {
                assert_eq!(name, "x");
            }
            assert!(matches!(value.0, Expr::Literal(Literal::Int(42))));
        } else {
            panic!("Expected assignment");
        }
    }

    #[test]
    fn test_parse_expr_stmt() {
        let result = parse_program("print(42)\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        assert!(matches!(stmts[0].0, Stmt::Expr(_)));
    }

    #[test]
    fn test_parse_return() {
        let result = parse_program("def foo():\n  return 42\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Def { body, .. } = &stmts[0].0 {
            assert_eq!(body.len(), 1);
            assert!(matches!(body[0].0, Stmt::Return(_)));
        } else {
            panic!("Expected def");
        }
    }

    #[test]
    fn test_parse_if() {
        let result = parse_program("if x > 0:\n  y = 1\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } = &stmts[0].0
        {
            assert!(matches!(
                condition.0,
                Expr::Binary {
                    op: BinaryOp::Greater,
                    ..
                }
            ));
            assert_eq!(then_block.len(), 1);
            assert_eq!(elif_blocks.len(), 0);
            assert!(else_block.is_none());
        } else {
            panic!("Expected if statement");
        }
    }

    #[test]
    fn test_parse_if_elif_else() {
        let source = "\
if x > 0:
  y = 1
elif x == 0:
  y = 0
else:
  y = -1
";
        let result = parse_program(source);
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::If {
            elif_blocks,
            else_block,
            ..
        } = &stmts[0].0
        {
            assert_eq!(elif_blocks.len(), 1);
            assert!(else_block.is_some());
        } else {
            panic!("Expected if statement");
        }
    }

    #[test]
    fn test_parse_while() {
        let result = parse_program("while x > 0:\n  x = x - 1\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::While { condition, body } = &stmts[0].0 {
            assert!(matches!(
                condition.0,
                Expr::Binary {
                    op: BinaryOp::Greater,
                    ..
                }
            ));
            assert_eq!(body.len(), 1);
        } else {
            panic!("Expected while statement");
        }
    }

    #[test]
    fn test_parse_def_no_params() {
        let result = parse_program("def foo():\n  return 42\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Def { name, params, body } = &stmts[0].0 {
            assert_eq!(name, "foo");
            assert_eq!(params.len(), 0);
            assert_eq!(body.len(), 1);
        } else {
            panic!("Expected def");
        }
    }

    #[test]
    fn test_parse_def_with_params() {
        let result = parse_program("def add(a, b):\n  return a + b\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Def { name, params, body } = &stmts[0].0 {
            assert_eq!(name, "add");
            assert_eq!(params.len(), 2);
            assert_eq!(params[0], "a");
            assert_eq!(params[1], "b");
            assert_eq!(body.len(), 1);
        } else {
            panic!("Expected def");
        }
    }

    #[test]
    fn test_parse_multiple_stmts() {
        let source = "\
x = 1
y = 2
z = x + y
";
        let result = parse_program(source);
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 3);
        assert!(matches!(stmts[0].0, Stmt::Assign { .. }));
        assert!(matches!(stmts[1].0, Stmt::Assign { .. }));
        assert!(matches!(stmts[2].0, Stmt::Assign { .. }));
    }

    #[test]
    fn test_parse_nested_blocks() {
        let source = "\
if x > 0:
  if y > 0:
    z = 1
  else:
    z = 2
";
        let result = parse_program(source);
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::If { then_block, .. } = &stmts[0].0 {
            assert_eq!(then_block.len(), 1);
            assert!(matches!(then_block[0].0, Stmt::If { .. }));
        } else {
            panic!("Expected if statement");
        }
    }

    // ========== 에러 복구 테스트 ==========

    #[test]
    fn test_parse_error_unclosed_paren() {
        let result = parse_expr("(1 + 2");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_error_invalid_syntax() {
        let result = parse_program("x = = 42\n");
        assert!(result.is_err());
    }
}
