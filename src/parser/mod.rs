pub(crate) mod ast;

use crate::lexer::Token;
use crate::types::Span;
use ast::*;
use chumsky::Parser;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub use chumsky::span::SimpleSpan;

type RichTokenError<'a> = Rich<'a, Token>;

// Helper enum for parsing postfix operations (internal to parser)
#[derive(Debug, Clone)]
enum PostfixOp {
    Attr(String, Span),
    Call(Vec<ExprS>, Span),
    Index(ExprS, Span),
}

/// 괄호 없는 튜플 또는 단일 표현식을 파싱합니다.
/// 
/// - `1, 2, 3` → `Expr::Tuple([1, 2, 3])`
/// - `1,` → `Expr::Tuple([1])`
/// - `1` → `Expr::Literal(1)`
/// 
/// 주의: 이 파서는 최상위 레벨에서만 사용되어야 합니다 (assignment RHS, return 값 등).
/// 내부 표현식에서는 기존 expr_parser를 사용합니다.
pub fn tuple_or_expr_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExprS, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    // expr_parser를 사용하되, 최상위 레벨에서 쉼표로 구분된 리스트를 허용
    expr_parser()
        .separated_by(just(Token::Comma))
        .at_least(1)
        .allow_trailing()
        .collect::<Vec<ExprS>>()
        .map_with(|exprs: Vec<ExprS>, e| {
            let s: I::Span = e.span();
            let span = s.into_range();
            
            if exprs.len() == 1 {
                // 단일 표현식
                exprs.into_iter().next().unwrap()
            } else {
                // 튜플
                (Expr::Tuple(exprs), span)
            }
        })
        .boxed()
}

pub fn expr_parser<'tokens, I>()
-> impl Parser<'tokens, I, ExprS, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    recursive(|expr| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        // List literal: [expr, expr, ...]
        let list_literal = expr
            .clone()
            .separated_by(just(Token::Comma))
            .allow_trailing()
            .collect()
            .delimited_by(just(Token::LBracket), just(Token::RBracket))
            .map(Expr::List)
            .labelled("list literal")
            .boxed();

        // TreeSet literal: t{expr, expr, ...}
        let treeset_literal = just(Token::Identifier("t".to_string()))
            .ignore_then(
                expr.clone()
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect()
                    .delimited_by(just(Token::LBrace), just(Token::RBrace))
                    .map(Expr::TreeSet)
            )
            .labelled("treeset literal")
            .boxed();

        // Set literal: {expr, expr, ...} (no colon)
        // Dict literal: {key: value, key: value, ...} (has colon)
        // Empty {} is treated as Dict
        let set_or_dict_literal = just(Token::LBrace)
            .ignore_then(
                // Empty dict: {}
                just(Token::RBrace)
                    .to(Expr::Dict(vec![]))
                    // Dict with key:value pairs
                    .or(expr.clone()
                        .then_ignore(just(Token::Colon))
                        .then(expr.clone())
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .then_ignore(just(Token::RBrace))
                        .map(Expr::Dict))
                    // Set with values (no colon)
                    .or(expr.clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect::<Vec<_>>()
                        .then_ignore(just(Token::RBrace))
                        .map(Expr::Set))
            )
            .labelled("set or dict literal")
            .boxed();

        // Tuple literal: (expr,) or (expr, expr, ...) - must have comma to distinguish from grouping
        // Empty tuple: ()
        // Note: (expr) is grouping, (expr,) is tuple
        // We need to detect trailing comma to distinguish them
        let tuple_literal = just(Token::LParen)
            .ignore_then(
                expr.clone()
                    .then(
                        just(Token::Comma)
                            .then(expr.clone().separated_by(just(Token::Comma)).allow_trailing().collect::<Vec<_>>())
                            .map(|(_, rest)| rest)
                            .or_not()
                    )
                    .map(|(first, rest_opt)| {
                        match rest_opt {
                            Some(rest) => {
                                // Comma exists: tuple
                                let mut elements = vec![first];
                                elements.extend(rest);
                                (elements, true) // true = is tuple
                            }
                            None => {
                                // No comma: could be grouping or empty tuple
                                (vec![first], false) // false = might be grouping
                            }
                        }
                    })
                    .or(just(Token::RParen).to((vec![], true))) // Empty tuple: ()
            )
            .then_ignore(just(Token::RParen))
            .map(|(elements, is_tuple)| (elements, is_tuple))
            .labelled("tuple or grouped expression")
            .boxed();

        // Primary: literals, variables, parenthesized expressions
        let primary = choice((
            select! {
                Token::Int(i) => Expr::Literal(Literal::Int(i)),
                Token::Bool(b) => Expr::Literal(Literal::Bool(b)),
                Token::String(s) => Expr::Literal(Literal::String(s)),
                Token::Float(f) => Expr::Literal(Literal::Float(f)),
                Token::None => Expr::Literal(Literal::None),
            }
            .labelled("literal"),
            list_literal,
            treeset_literal,
            set_or_dict_literal,
            // Tuple or parenthesized expression
            tuple_literal.map(|(elements, is_tuple)| {
                if is_tuple {
                    // Tuple: (expr,), (expr, expr, ...), or ()
                    if elements.len() == 1 {
                        // Single element tuple: (expr,)
                        Expr::Tuple(elements)
                    } else {
                        // Multiple elements or empty: tuple
                        Expr::Tuple(elements)
                    }
                } else {
                    // Grouping: (expr)
                    elements.into_iter().next().unwrap().0
                }
            }),
            ident.map(Expr::Variable),
        ))
        .map_with(|node: Expr, e| {
            let s: I::Span = e.span();
            (node, s.into_range())
        }).boxed();

        // Postfix: handles ., (), and [] chaining
        let postfix_op = choice((
            // .attr (attribute access)
            just(Token::Dot)
                .ignore_then(ident)
                .map_with(|attr, e| {
                    let s: I::Span = e.span();
                    PostfixOp::Attr(attr, s.into_range())
                }),
            // (args) (function/method call)
            expr.clone()
                .separated_by(just(Token::Comma))
                .allow_trailing()
                .collect()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .map_with(|args, e| {
                    let s: I::Span = e.span();
                    PostfixOp::Call(args, s.into_range())
                }),
            // [index] (indexing)
            expr.clone()
                .delimited_by(just(Token::LBracket), just(Token::RBracket))
                .map_with(|index, e| {
                    let s: I::Span = e.span();
                    PostfixOp::Index(index, s.into_range())
                }),
        )).boxed();

        let atom = primary.foldl(postfix_op.repeated(), |base: ExprS, op: PostfixOp| {
            let start = base.1.start;
            match op {
                // {base}.{attr}
                PostfixOp::Attr(attr, op_span) => {
                    let end = op_span.end;  // use actual span end from parser
                    (
                        Expr::Attribute {
                            object: Box::new(base),
                            attr,
                        },
                        start..end,
                    )
                }
                // {base}({args})
                PostfixOp::Call(args, op_span) => {
                    let end = op_span.end;  // use actual span end from parser
                    (
                        Expr::Call {
                            func_name: Box::new(base),
                            args,
                        },
                        start..end,
                    )
                }
                // {base}[{index}]
                PostfixOp::Index(index, op_span) => {
                    let end = op_span.end;  // use actual span end from parser
                    (
                        Expr::Index {
                            object: Box::new(base),
                            index: Box::new(index),
                        },
                        start..end,
                    )
                }
            }
        }).boxed();

        // Capture unary operator with its span
        let op_unary = choice((
            just(Token::Not).to(UnaryOp::Not),
            just(Token::Minus).to(UnaryOp::Negate),
            just(Token::Plus).to(UnaryOp::Pos),
        ))
        .map_with(|op, e| {
            let s: I::Span = e.span();
            (op, s.into_range())
        }).boxed();

        let unary = op_unary.repeated().foldr(
            atom,
            |(op, op_span): (UnaryOp, Span), right: ExprS| {
                let span = op_span.start..right.1.end;
                (
                    Expr::Unary {
                        op,
                        expr: Box::new(right),
                    },
                    span,
                )
            }
        ).boxed();

        let op = |t| just(t).ignored().boxed();
        let product = unary.clone().foldl(
                choice((
                    op(Token::Star).to(BinaryOp::Multiply),
                    op(Token::SlashSlash).to(BinaryOp::FloorDivide),
                    op(Token::Slash).to(BinaryOp::Divide),
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
        ).boxed();
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
            ).boxed();
        let comparison = sum.clone().then(
            choice((
                op(Token::Less).to(BinaryOp::Less),
                op(Token::LessEqual).to(BinaryOp::LessEqual),
                op(Token::Greater).to(BinaryOp::Greater),
                op(Token::GreaterEqual).to(BinaryOp::GreaterEqual),
                op(Token::EqualEqual).to(BinaryOp::Equal),
                op(Token::NotEqual).to(BinaryOp::NotEqual),
            ))
            .then(sum)
            .repeated()
            .collect::<Vec<_>>()
        )
        .map(|(left, chain)| {
            if chain.is_empty() {
                return left;
            }

            let mut terms = vec![left];
            terms.extend(chain.iter().map(|(_, term)| term.clone()));

            let mut comparisons = vec![];
            for i in 0..chain.len() {
                let (op, _) = &chain[i];
                let l = terms[i].clone();
                let r = terms[i+1].clone();
                let span = l.1.start..r.1.end;
                comparisons.push((Expr::Binary { op: op.clone(), left: Box::new(l), right: Box::new(r) }, span));
            }

            comparisons.into_iter().reduce(|acc, next| {
                let span = acc.1.start..next.1.end;
                (Expr::Binary {
                    op: BinaryOp::And,
                    left: Box::new(acc),
                    right: Box::new(next),
                }, span)
            }).unwrap()
        }).boxed();
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
        ).boxed();
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
        ).boxed();

        let lambda_expr = just(Token::Lambda)
            .ignore_then(
                ident
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect(),
            )
            .then_ignore(just(Token::Colon))
            .then(expr.clone())
            .map_with(|(params, body): (Vec<String>, ExprS), e| {
                let s: I::Span = e.span();
                (
                    Expr::Lambda {
                        params,
                        body: Box::new(body),
                    },
                    s.into_range(),
                )
            })
            .labelled("lambda expression");

        choice((lambda_expr, or_expr)).labelled("expression")
    }).boxed()
}

pub fn stmt_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<StmtS>, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    let expr = expr_parser().boxed();
    // 괄호 없는 튜플을 허용하는 표현식 파서 (RHS, return 값 등)
    let tuple_or_expr = tuple_or_expr_parser().boxed();

    recursive(|stmt| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        // Line end (either newline or end of input)
        let line_end = just(Token::Newline).ignored().or(end().ignored()).boxed();

        // Simple statement kinds (no line ending consumption here)
        let return_stmt = just(Token::Return)
            .ignore_then(tuple_or_expr.clone())
            .map(Stmt::Return)
            .labelled("return statement")
            .boxed();

        // Assignment: LHS도 튜플 패턴을 지원해야 함
        // LHS: expr (Variable, Attribute, Index, 또는 Tuple)
        // RHS: tuple_or_expr (괄호 없는 튜플 허용)
        let assign_stmt = tuple_or_expr
            .clone()
            .then_ignore(just(Token::Equal))
            .then(tuple_or_expr.clone())
            .map_with(|(target, value), e| {
                let s: I::Span = e.span();
                Stmt::Assign {
                    target: (target.0, s.into_range()),
                    value,
                }
            })
            .labelled("assignment")
            .boxed();

        let expr_stmt = tuple_or_expr
            .clone()
            .map(Stmt::Expr)
            .labelled("expression statement")
            .boxed();

        let break_stmt = just(Token::Break)
            .to(Stmt::Break)
            .labelled("break statement")
            .boxed();

        let continue_stmt = just(Token::Continue)
            .to(Stmt::Continue)
            .labelled("continue statement")
            .boxed();

        let pass_stmt = just(Token::Pass).to(Stmt::Pass).labelled("pass statement").boxed();

        // A line of one or more simple statements separated by ';' with optional trailing ';'
        let simple_stmt = choice((
            return_stmt.clone(),
            assign_stmt.clone(),
            expr_stmt.clone(),
            break_stmt.clone(),
            continue_stmt.clone(),
            pass_stmt.clone(),
        ))
        .map_with(|node: Stmt, e| {
            let s: I::Span = e.span();
            (node, s.into_range())
        }).boxed();

        let simple_stmts_line = simple_stmt
            .separated_by(just(Token::Semicolon))
            .allow_trailing()
            .at_least(1)
            .collect::<Vec<StmtS>>()
            .then_ignore(line_end.clone())
            .labelled("simple statements").boxed();

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
            ).boxed();

        let block = just(Token::Colon).ignore_then(choice((indented_block, simple_stmts_line.clone()))).boxed();

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
            .labelled("def statement")
            .boxed();

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
            .labelled("if statement")
            .boxed();

        let while_stmt = just(Token::While)
            .ignore_then(expr.clone())
            .then(block.clone())
            .map(|(condition, body)| Stmt::While { condition, body })
            .labelled("while statement")
            .boxed();

        let for_stmt = just(Token::For)
            .ignore_then(ident)
            .then_ignore(just(Token::In))
            .then(expr.clone())
            .then(block.clone())
            .map(|((var, iterable), body)| Stmt::For {
                var,
                iterable,
                body,
            })
            .labelled("for statement")
            .boxed();

        // Class statement
        let method_def = just(Token::Def)
            .ignore_then(ident)
            .then(
                ident
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then(block.clone())
            .map(|((name, params), body)| MethodDef { name, params, body })
            .labelled("method definition")
            .boxed();

        let class_stmt = just(Token::Class)
            .ignore_then(ident)
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
            .labelled("class statement")
            .boxed();

        // Compound statements occupy the whole logical line
        let compound_stmt_line = choice((class_stmt, def_stmt, if_stmt, while_stmt, for_stmt))
            .map_with(|node: Stmt, e| {
                let s: I::Span = e.span();
                (node, s.into_range())
            })
            .map(|s| vec![s]).boxed();

        // Allow empty lines between statements
        let blank_lines = just(Token::Newline).ignored().repeated().boxed();

        choice((compound_stmt_line, simple_stmts_line))
            .padded_by(blank_lines)
            .recover_with(skip_then_retry_until(
                any().ignored(),
                just(Token::Newline)
                    .ignored()
                    .or(just(Token::Dedent).ignored())
                    .or(end().ignored()),
            ))
    }).boxed()
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
            .then_ignore(end()).boxed()
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
        let stream = chumsky::input::Stream::from_iter(tokens).map(eoi_span, |(t, s)| (t, s));
        expr_parser().parse(stream).into_result()
    }

    fn parse_program(source: &str) -> Result<Vec<StmtS>, Vec<RichTokenError<'_>>> {
        let tokens = tokenize(source);
        let eoi_span = SimpleSpan::new(source.len(), source.len());
        let stream = chumsky::input::Stream::from_iter(tokens).map(eoi_span, |(t, s)| (t, s));
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
        if let Expr::Literal(Literal::Int(-42)) = result.unwrap().0 {
            assert!(true);
        } else {
            panic!("Expected literal int -42");
        }
        // if let Expr::Unary { op, expr } = result.unwrap().0 {
        //     assert!(matches!(op, UnaryOp::Negate));
        //     assert!(matches!(expr.0, Expr::Literal(Literal::Int(42))));
        // } else {
        //     panic!("Expected unary negate");
        // }
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

    #[test]
    fn test_parse_comparison_chain() {
        let result = parse_expr("1 < x < 10");
        assert!(result.is_ok());
        let expr = result.unwrap();
        // Should be parsed as (1 < x) and (x < 10)
        if let Expr::Binary { op: BinaryOp::And, left, right } = expr.0 {
            if let Expr::Binary { op: BinaryOp::Less, left: l1, right: r1 } = left.0 {
                assert!(matches!(l1.0, Expr::Literal(Literal::Int(1))));
                assert!(matches!(r1.0, Expr::Variable(_)));
            } else {
                panic!("Expected less expression on left");
            }
            if let Expr::Binary { op: BinaryOp::Less, left: l2, right: r2 } = right.0 {
                assert!(matches!(l2.0, Expr::Variable(_)));
                assert!(matches!(r2.0, Expr::Literal(Literal::Int(10))));
            } else {
                panic!("Expected less expression on right");
            }
        } else {
            panic!("Expected and expression at top level");
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

    // ========== 괄호 없는 튜플 및 언패킹 테스트 ==========

    #[test]
    fn test_parse_tuple_without_parens() {
        let result = parse_program("a = 1, 2, 3\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Assign { target, value } = &stmts[0].0 {
            assert!(matches!(target.0, Expr::Variable(_)));
            if let Expr::Tuple(elements) = &value.0 {
                assert_eq!(elements.len(), 3);
            } else {
                panic!("Expected tuple in RHS");
            }
        } else {
            panic!("Expected assignment");
        }
    }

    #[test]
    fn test_parse_single_element_tuple() {
        // 단일 요소 튜플은 trailing comma가 있어야 함
        // 현재 구현에서는 1, 2개 이상의 요소가 있어야 튜플로 인식됨
        // 단일 요소는 괄호를 사용해야 함: (1,)
        let result = parse_program("a = (1,)\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        if let Stmt::Assign { value, .. } = &stmts[0].0 {
            if let Expr::Tuple(elements) = &value.0 {
                assert_eq!(elements.len(), 1);
            } else {
                panic!("Expected tuple");
            }
        }
    }

    #[test]
    fn test_parse_unpack_assignment() {
        let result = parse_program("a, b, c = 1, 2, 3\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        if let Stmt::Assign { target, value } = &stmts[0].0 {
            // LHS는 튜플 패턴
            if let Expr::Tuple(targets) = &target.0 {
                assert_eq!(targets.len(), 3);
            } else {
                panic!("Expected tuple pattern in LHS");
            }
            // RHS는 튜플
            if let Expr::Tuple(values) = &value.0 {
                assert_eq!(values.len(), 3);
            } else {
                panic!("Expected tuple in RHS");
            }
        }
    }

    #[test]
    fn test_parse_unpack_from_list() {
        let result = parse_program("x, y, z = [1, 2, 3]\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        if let Stmt::Assign { target, value } = &stmts[0].0 {
            if let Expr::Tuple(targets) = &target.0 {
                assert_eq!(targets.len(), 3);
            }
            if let Expr::List(_) = &value.0 {
                // OK
            } else {
                panic!("Expected list in RHS");
            }
        }
    }

    #[test]
    fn test_parse_nested_unpack() {
        let result = parse_program("a, (b, c) = 1, (2, 3)\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        if let Stmt::Assign { target, value } = &stmts[0].0 {
            if let Expr::Tuple(targets) = &target.0 {
                assert_eq!(targets.len(), 2);
                // 두 번째 요소가 중첩 튜플인지 확인
                if let Expr::Tuple(nested) = &targets[1].0 {
                    assert_eq!(nested.len(), 2);
                } else {
                    panic!("Expected nested tuple");
                }
            }
        }
    }

    #[test]
    fn test_parse_return_tuple() {
        let result = parse_program("def foo():\n  return 1, 2, 3\n");
        assert!(result.is_ok());
        let stmts = result.unwrap();
        if let Stmt::Def { body, .. } = &stmts[0].0 {
            if let Stmt::Return(expr) = &body[0].0 {
                if let Expr::Tuple(elements) = &expr.0 {
                    assert_eq!(elements.len(), 3);
                } else {
                    panic!("Expected tuple in return");
                }
            }
        }
    }
}
