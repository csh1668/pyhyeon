pub(crate) mod ast;

use crate::lexer::Token;
use ast::*;
use chumsky::Parser;
use chumsky::input::ValueInput;
use chumsky::prelude::*;

pub use chumsky::span::SimpleSpan;

type RichError<'a> = Rich<'a, Token>;

pub fn expr_parser<'tokens, I>() -> impl Parser<'tokens, I, Expr, extra::Err<RichError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    recursive(|expr| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        let atom = choice((
            select! {
                Token::Int(i) => Expr::Literal(Literal::Int(i)),
                Token::Bool(b) => Expr::Literal(Literal::Bool(b)),
            }
            .labelled("literal"),
            ident
                .clone()
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
                .delimited_by(just(Token::LParen), just(Token::RParen)),
        ));

        let unary = just(Token::Not)
            .to(UnaryOp::Not)
            .or(just(Token::Minus).to(UnaryOp::Negate))
            .or(just(Token::Plus).to(UnaryOp::Pos))
            .repeated()
            .foldr(atom, |op, right| Expr::Unary {
                op,
                expr: Box::new(right),
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
            |left, (op, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        );
        let sum = product.clone().foldl(
            choice((
                op(Token::Plus).to(BinaryOp::Add),
                op(Token::Minus).to(BinaryOp::Subtract),
            ))
            .then(product)
            .repeated(),
            |left, (op, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
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
            |left, (op, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        );
        let and_expr = comparison.clone().foldl(
            op(Token::And).to(BinaryOp::And).then(comparison).repeated(),
            |left, (op, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        );
        let or_expr = and_expr.clone().foldl(
            op(Token::Or).to(BinaryOp::Or).then(and_expr).repeated(),
            |left, (op, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
        );

        or_expr.labelled("expression")
    })
    .boxed()
}

pub fn stmt_parser<'tokens, I>() -> impl Parser<'tokens, I, Stmt, extra::Err<RichError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    let expr = expr_parser().boxed();

    recursive(|stmt| {
        let ident = select! { Token::Identifier(s) => s }.labelled("identifier");

        // Helper: parse a block after a ':'
        let block = stmt.clone().repeated().collect().delimited_by(
            just(Token::Colon)
                .ignore_then(just(Token::Newline))
                .ignore_then(just(Token::Indent)),
            just(Token::Dedent),
        );

        let def_stmt = just(Token::Def)
            .ignore_then(ident.clone())
            .then(
                ident
                    .separated_by(just(Token::Comma))
                    .allow_trailing()
                    .collect()
                    .delimited_by(just(Token::LParen), just(Token::RParen)),
            )
            .then(block.clone())
            .map(|((name, params), body)| Stmt::Def { name, params, body })
            .labelled("def statement");

        let if_stmt = just(Token::If)
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(
                (just(Token::Elif)
                    .ignore_then(expr.clone())
                    .then(block.clone()))
                .repeated()
                .collect(),
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

        let line_end = just(Token::Newline).ignored().or(end().ignored()).or(just(Token::Eof).ignored());

        let return_stmt = just(Token::Return)
            .ignore_then(expr.clone())
            .then_ignore(line_end.clone())
            .map(Stmt::Return)
            .labelled("return statement");

        let assign_stmt = ident
            .clone()
            .then_ignore(just(Token::Equal))
            .then(expr.clone())
            .then_ignore(line_end.clone())
            .map(|(name, value)| Stmt::Assign { name, value })
            .labelled("assignment");

        let expr_stmt = expr
            .clone()
            .then_ignore(line_end.clone())
            .map(Stmt::Expr)
            .labelled("expression statement");

        // Allow empty lines between statements
        let blank_lines = just(Token::Newline).ignored().repeated();

        choice((
            def_stmt,
            if_stmt,
            while_stmt,
            return_stmt,
            assign_stmt,
            expr_stmt,
        ))
        .padded_by(blank_lines)
        .labelled("statement")
    })
    .boxed()
}

pub fn program_parser<'tokens, I>()
-> impl Parser<'tokens, I, Vec<Stmt>, extra::Err<RichError<'tokens>>>
where
    I: ValueInput<'tokens, Token = Token, Span = SimpleSpan> + 'tokens,
{
    let stmt = stmt_parser().boxed();
    let blanks = just(Token::Newline).ignored().repeated();

    blanks
        .clone()
        .ignore_then(stmt.clone().repeated().collect())
        .then_ignore(blanks)
        .then_ignore(end())
        .boxed()
}
