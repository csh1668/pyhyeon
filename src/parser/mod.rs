pub(crate) mod ast;

use crate::lexer::Token;
use crate::types::Span;
use ast::*;
use chumsky::Parser;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub use chumsky::span::SimpleSpan;

type RichTokenError<'a> = Rich<'a, Token>;

pub fn expr_parser<'tokens, I>() -> impl Parser<'tokens, I, ExprS, extra::Err<RichTokenError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    recursive(|expr| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        let atom = choice((
            select! {
                Token::Int(i) => Expr::Literal(Literal::Int(i)),
                Token::Bool(b) => Expr::Literal(Literal::Bool(b)),
                Token::None => Expr::Literal(Literal::None),
            }
            .labelled("literal"),
            ident
                .then(
                    expr.clone()
                        .separated_by(just(Token::Comma))
                        .allow_trailing()
                        .collect()
                        .delimited_by(just(Token::LParen), just(Token::RParen)),
                )
                .map(|(func_name, args)| Expr::Call { func_name, args }),
            ident.map(Expr::Variable),
            expr.clone()
                .delimited_by(just(Token::LParen), just(Token::RParen))
                .map(|e| e.0), // take inner expr node; span will be overwritten below
        ))
        .map_with(|node: Expr, e| {
            let s: I::Span = e.span();
            (node, s.into_range())
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

pub fn stmt_parser<'tokens, I>() -> impl Parser<'tokens, I, Vec<StmtS>, extra::Err<RichTokenError<'tokens>>>
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

        let assign_stmt = ident
            .then_ignore(just(Token::Equal))
            .then(expr.clone())
            .map(|(name, value)| Stmt::Assign { name, value })
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

        let block = just(Token::Colon)
            .ignore_then(choice((indented_block, simple_stmts_line.clone())));

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

        // Compound statements occupy the whole logical line
        let compound_stmt_line = choice((def_stmt, if_stmt, while_stmt))
            .map_with(|node: Stmt, e| {
                let s: I::Span = e.span();
                (node, s.into_range())
            })
            .map(|s| vec![s]);

        // Allow empty lines between statements
        let blank_lines = just(Token::Newline).ignored().repeated();

        choice((compound_stmt_line, simple_stmts_line))
            .padded_by(blank_lines)
            .recover_with(skip_then_retry_until(any().ignored(), just(Token::Newline).ignored().or(just(Token::Dedent).ignored()).or(end().ignored())))
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
        .ignore_then(line.clone().repeated().collect::<Vec<Vec<StmtS>>>() )
        .map(|lines| lines.into_iter().flatten().collect::<Vec<StmtS>>())
        .then_ignore(blanks)
        .then_ignore(end())
        .boxed()
}
