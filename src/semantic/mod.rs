pub mod scope;
pub mod typecheck;

use std::collections::{HashMap, HashSet};

use crate::parser::ast::{Expr, ExprS, Stmt, StmtS};
use crate::types::Span;

#[derive(Debug)]
pub struct SemanticError {
    pub message: String,
    pub span: Span,
}

pub type SemanticResult<T> = Result<T, SemanticError>;

#[derive(Default)]
pub struct ProgramContext {
    builtins: HashSet<String>,
    functions: HashMap<String, usize>, // name -> arity
}

impl ProgramContext {
    fn new_with_builtins() -> Self {
        let mut ctx = Self::default();
        for b in crate::builtins::all() {
            ctx.builtins.insert(b.name.to_string());
        }
        ctx
    }

    fn is_builtin(&self, name: &str) -> bool {
        self.builtins.contains(name)
    }
}

pub fn analyze(program: &[StmtS]) -> SemanticResult<()> {
    analyze_with_globals(program, &[])
}

/// REPL용: 기존 전역 변수를 포함하여 분석
pub fn analyze_with_globals(program: &[StmtS], existing_globals: &[String]) -> SemanticResult<()> {
    // 1) 이름 해석(스코프) + 간단 규칙 확인
    let mut ctx = ProgramContext::new_with_builtins();
    let mut scopes = scope::ScopeStack::new();
    // preload builtins into global scope for resolution
    for b in ctx.builtins.clone() {
        scopes.define(b);
    }

    // preload existing globals (for REPL)
    for g in existing_globals {
        scopes.define(g.clone());
    }

    // 모듈 레벨 분석
    for stmt in program {
        analyze_stmt_module(stmt, &mut scopes, &mut ctx)?;
    }

    // 2) 타입 검사
    typecheck::typecheck_program(program, &ctx)?;
    Ok(())
}

fn analyze_stmt_module(
    stmt: &StmtS,
    scopes: &mut scope::ScopeStack,
    ctx: &mut ProgramContext,
) -> SemanticResult<()> {
    match &stmt.0 {
        Stmt::Break | Stmt::Continue | Stmt::Pass => {
            // Break/Continue validation is done in typecheck
            // Pass is always allowed as a no-op
            Ok(())
        }
        Stmt::Def { name, params, body } => {
            // 정의는 현재 스코프(모듈)에 바인딩
            scopes.define(name.clone());
            ctx.functions.insert(name.clone(), params.len());
            analyze_function(name, params, body, scopes, ctx, stmt.1.clone())
        }
        Stmt::Assign { target, value } => {
            analyze_expr_module(value, scopes, ctx)?;
            // target이 Variable이면 정의, Attribute나 Index이면 object만 검증
            match &target.0 {
                Expr::Variable(name) => {
                    if !scopes.is_defined(name) {
                        scopes.define(name.clone());
                    }
                }
                Expr::Attribute { .. } | Expr::Index { .. } => {
                    analyze_expr_module(target, scopes, ctx)?;
                }
                _ => {
                    return Err(SemanticError {
                        message: "Invalid assignment target".to_string(),
                        span: target.1.clone(),
                    });
                }
            }
            Ok(())
        }
        Stmt::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } => {
            analyze_expr_module(condition, scopes, ctx)?;
            // 블록 스코프는 만들지 않는다
            for s in then_block {
                analyze_stmt_module(s, scopes, ctx)?;
            }
            for (cond, block) in elif_blocks {
                analyze_expr_module(cond, scopes, ctx)?;
                for s in block {
                    analyze_stmt_module(s, scopes, ctx)?;
                }
            }
            if let Some(block) = else_block {
                for s in block {
                    analyze_stmt_module(s, scopes, ctx)?;
                }
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            analyze_expr_module(condition, scopes, ctx)?;
            for s in body {
                analyze_stmt_module(s, scopes, ctx)?;
            }
            Ok(())
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            // iterable 표현식 분석
            analyze_expr_module(iterable, scopes, ctx)?;

            // 루프 변수를 현재 스코프에 정의
            scopes.define(var.clone());

            // body 분석
            for s in body {
                analyze_stmt_module(s, scopes, ctx)?;
            }
            Ok(())
        }
        Stmt::Return(expr) => analyze_expr_module(expr, scopes, ctx),
        Stmt::Expr(expr) => analyze_expr_module(expr, scopes, ctx),
        Stmt::Class { name, methods, .. } => {
            // 클래스를 현재 스코프에 정의
            scopes.define(name.clone());

            // 각 메서드 검증
            for method in methods {
                // 첫 번째 파라미터가 self인지 확인 (__init__ 포함)
                if method.params.is_empty() || method.params[0] != "self" {
                    return Err(SemanticError {
                        message: format!(
                            "Method '{}' in class '{}' must have 'self' as first parameter",
                            method.name, name
                        ),
                        span: stmt.1.clone(),
                    });
                }

                // 메서드 본문 분석
                scopes.push();
                for param in &method.params {
                    scopes.define(param.clone());
                }

                let locals: HashSet<String> = method.params.iter().cloned().collect();
                let mut assigned: HashSet<String> = method.params.iter().cloned().collect();

                for s in &method.body {
                    analyze_stmt_function(s, scopes, ctx, &locals, &mut assigned)?;
                }

                scopes.pop();
            }

            Ok(())
        }
    }
}

fn analyze_expr_module(
    expr: &ExprS,
    scopes: &mut scope::ScopeStack,
    ctx: &ProgramContext,
) -> SemanticResult<()> {
    match &expr.0 {
        Expr::Literal(_) => Ok(()),
        Expr::Variable(name) => {
            if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                return Err(SemanticError {
                    message: format!("Undefined variable: {}", name),
                    span: expr.1.clone(),
                });
            }
            Ok(())
        }
        Expr::Unary { op: _, expr: inner } => analyze_expr_module(inner, scopes, ctx),
        Expr::Binary { op: _, left, right } => {
            analyze_expr_module(left, scopes, ctx)?;
            analyze_expr_module(right, scopes, ctx)
        }
        Expr::Call { func_name, args } => {
            // func_name이 Variable인 경우만 체크
            if let Expr::Variable(name) = &func_name.0 {
                if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                    return Err(SemanticError {
                        message: format!("Undefined function: {}", name),
                        span: expr.1.clone(),
                    });
                }
            } else {
                // Attribute 등 다른 경우는 func_name 자체를 분석
                analyze_expr_module(func_name, scopes, ctx)?;
            }
            for a in args {
                analyze_expr_module(a, scopes, ctx)?;
            }
            Ok(())
        }
        Expr::Attribute { object, .. } => {
            analyze_expr_module(object, scopes, ctx)?;
            Ok(())
        }
        Expr::List(elements) => {
            for elem in elements {
                analyze_expr_module(elem, scopes, ctx)?;
            }
            Ok(())
        }
        Expr::Dict(pairs) => {
            for (key, value) in pairs {
                analyze_expr_module(key, scopes, ctx)?;
                analyze_expr_module(value, scopes, ctx)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            analyze_expr_module(object, scopes, ctx)?;
            analyze_expr_module(index, scopes, ctx)?;
            Ok(())
        }
        Expr::Lambda { params, body } => {
            // Check for unbound captured variables
            let mut free_vars = HashSet::new();
            collect_free_vars(body, params, &mut free_vars);

            for var in &free_vars {
                if !scopes.is_defined(var) && !ctx.is_builtin(var) {
                    return Err(SemanticError {
                        message: format!("Undefined variable '{}' captured by lambda", var),
                        span: expr.1.clone(),
                    });
                }
            }

            // Analyze the lambda body in a new scope
            scopes.push();
            for p in params {
                scopes.define(p.clone());
            }
            analyze_expr_module(body, scopes, ctx)?;
            scopes.pop();
            Ok(())
        }
    }
}

fn analyze_function(
    _name: &str,
    params: &Vec<String>,
    body: &Vec<StmtS>,
    scopes: &mut scope::ScopeStack,
    ctx: &mut ProgramContext,
    fn_span: Span,
) -> SemanticResult<()> {
    // 함수 스코프 시작
    scopes.push();
    for p in params {
        scopes.define(p.clone());
    }

    // 로컬 판정: 파라미터 + 함수 내부의 모든 Assign/Def 이름
    let mut locals: HashSet<String> = params.iter().cloned().collect();
    collect_locals(body, &mut locals);

    // 할당 추적(초기: 파라미터)
    let mut assigned: HashSet<String> = params.iter().cloned().collect();

    for s in body {
        analyze_stmt_function(s, scopes, ctx, &locals, &mut assigned)?;
    }

    scopes.pop();
    // 함수 자체에 대한 추가 규칙은 타입체커에서 수행
    let _ = fn_span; // reserved
    Ok(())
}

fn collect_locals(body: &Vec<StmtS>, locals: &mut HashSet<String>) {
    for s in body {
        match &s.0 {
            Stmt::Assign { target, .. } => {
                // locals는 Variable만 추적
                if let Expr::Variable(name) = &target.0 {
                    locals.insert(name.clone());
                }
            }
            Stmt::Def { name, .. } => {
                locals.insert(name.clone());
            }
            Stmt::Class { name, .. } => {
                locals.insert(name.clone());
            }
            Stmt::If {
                then_block,
                elif_blocks,
                else_block,
                ..
            } => {
                collect_locals(then_block, locals);
                for (_, block) in elif_blocks {
                    collect_locals(block, locals);
                }
                if let Some(b) = else_block {
                    collect_locals(b, locals);
                }
            }
            Stmt::While { body, .. } => {
                collect_locals(body, locals);
            }
            Stmt::For { var, body, .. } => {
                // 루프 변수를 local로 수집
                locals.insert(var.clone());
                collect_locals(body, locals);
            }
            Stmt::Break | Stmt::Continue | Stmt::Pass => {}
            Stmt::Return(_) | Stmt::Expr(_) => {}
        }
    }
}

fn collect_free_vars(expr: &ExprS, params: &Vec<String>, free_vars: &mut HashSet<String>) {
    match &expr.0 {
        Expr::Variable(name) => {
            if !params.contains(name) {
                free_vars.insert(name.clone());
            }
        }
        Expr::Literal(_) => {}
        Expr::Unary { expr, .. } => collect_free_vars(expr, params, free_vars),
        Expr::Binary { left, right, .. } => {
            collect_free_vars(left, params, free_vars);
            collect_free_vars(right, params, free_vars);
        }
        Expr::Call { func_name, args } => {
            collect_free_vars(func_name, params, free_vars);
            for arg in args {
                collect_free_vars(arg, params, free_vars);
            }
        }
        Expr::Attribute { object, .. } => {
            collect_free_vars(object, params, free_vars);
        }
        Expr::List(elements) => {
            for elem in elements {
                collect_free_vars(elem, params, free_vars);
            }
        }
        Expr::Dict(pairs) => {
            for (key, value) in pairs {
                collect_free_vars(key, params, free_vars);
                collect_free_vars(value, params, free_vars);
            }
        }
        Expr::Index { object, index } => {
            collect_free_vars(object, params, free_vars);
            collect_free_vars(index, params, free_vars);
        }
        Expr::Lambda {
            params: inner_params,
            body,
        } => {
            let mut inner_free_vars = HashSet::new();
            collect_free_vars(body, inner_params, &mut inner_free_vars);
            for var in inner_free_vars {
                if !params.contains(&var) {
                    free_vars.insert(var);
                }
            }
        }
    }
}

fn analyze_stmt_function(
    stmt: &StmtS,
    scopes: &mut scope::ScopeStack,
    ctx: &ProgramContext,
    locals: &HashSet<String>,
    assigned: &mut HashSet<String>,
) -> SemanticResult<()> {
    match &stmt.0 {
        Stmt::Break | Stmt::Continue | Stmt::Pass => {
            // Break/Continue validation is done in typecheck
            // Pass is always allowed as a no-op
            Ok(())
        }
        Stmt::Assign { target, value } => {
            analyze_expr_function(value, scopes, ctx, locals, assigned)?;
            // target이 Variable이면 정의, Attribute나 Index이면 object만 검증
            match &target.0 {
                Expr::Variable(name) => {
                    assigned.insert(name.clone());
                    if !scopes.is_defined(name) {
                        scopes.define(name.clone());
                    }
                }
                Expr::Attribute { .. } | Expr::Index { .. } => {
                    analyze_expr_function(target, scopes, ctx, locals, assigned)?;
                }
                _ => {
                    return Err(SemanticError {
                        message: "Invalid assignment target".to_string(),
                        span: target.1.clone(),
                    });
                }
            }
            Ok(())
        }
        Stmt::Def { name, params, body } => {
            // 함수 정의도 로컬에 바인딩
            if !scopes.is_defined(name) {
                scopes.define(name.clone());
            }
            // 중첩 함수: 캡처 미지원 → 내부에서 바깥 로컬 참조 시 이후 타입/이름 단계에서 오류가 날 수 있음
            let mut inner_ctx = ProgramContext {
                builtins: ctx.builtins.clone(),
                functions: ctx.functions.clone(),
            };
            inner_ctx.functions.insert(name.clone(), params.len());
            analyze_function(name, params, body, scopes, &mut inner_ctx, stmt.1.clone())
        }
        Stmt::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } => {
            analyze_expr_function(condition, scopes, ctx, locals, assigned)?;
            for s in then_block {
                analyze_stmt_function(s, scopes, ctx, locals, assigned)?;
            }
            for (cond, block) in elif_blocks {
                analyze_expr_function(cond, scopes, ctx, locals, assigned)?;
                for s in block {
                    analyze_stmt_function(s, scopes, ctx, locals, assigned)?;
                }
            }
            if let Some(block) = else_block {
                for s in block {
                    analyze_stmt_function(s, scopes, ctx, locals, assigned)?;
                }
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            analyze_expr_function(condition, scopes, ctx, locals, assigned)?;
            for s in body {
                analyze_stmt_function(s, scopes, ctx, locals, assigned)?;
            }
            Ok(())
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            // iterable 표현식 분석
            analyze_expr_function(iterable, scopes, ctx, locals, assigned)?;

            // 루프 변수를 스코프에 정의
            if !scopes.is_defined(var) {
                scopes.define(var.clone());
            }

            // 루프 변수가 할당됨을 표시
            if locals.contains(var) {
                assigned.insert(var.clone());
            }

            // body 분석
            for s in body {
                analyze_stmt_function(s, scopes, ctx, locals, assigned)?;
            }
            Ok(())
        }
        Stmt::Return(expr) => analyze_expr_function(expr, scopes, ctx, locals, assigned),
        Stmt::Expr(expr) => analyze_expr_function(expr, scopes, ctx, locals, assigned),
        Stmt::Class { name, .. } => {
            // 함수 내부에서 클래스 정의는 로컬 변수로 취급
            scopes.define(name.clone());
            Ok(())
        }
    }
}

fn analyze_expr_function(
    expr: &ExprS,
    scopes: &mut scope::ScopeStack,
    ctx: &ProgramContext,
    locals: &HashSet<String>,
    assigned: &HashSet<String>,
) -> SemanticResult<()> {
    match &expr.0 {
        Expr::Literal(_) => Ok(()),
        Expr::Variable(name) => {
            if locals.contains(name) {
                if !assigned.contains(name) {
                    return Err(SemanticError {
                        message: format!("Unbound local variable: {}", name),
                        span: expr.1.clone(),
                    });
                }
                return Ok(());
            }
            if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                return Err(SemanticError {
                    message: format!("Undefined variable: {}", name),
                    span: expr.1.clone(),
                });
            }
            Ok(())
        }
        Expr::Unary { op: _, expr: inner } => {
            analyze_expr_function(inner, scopes, ctx, locals, assigned)
        }
        Expr::Binary { op: _, left, right } => {
            analyze_expr_function(left, scopes, ctx, locals, assigned)?;
            analyze_expr_function(right, scopes, ctx, locals, assigned)
        }
        Expr::Call { func_name, args } => {
            // func_name이 Variable인 경우만 체크
            if let Expr::Variable(name) = &func_name.0 {
                if locals.contains(name) {
                    if !assigned.contains(name) {
                        return Err(SemanticError {
                            message: format!("Unbound local function: {}", name),
                            span: expr.1.clone(),
                        });
                    }
                } else if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                    return Err(SemanticError {
                        message: format!("Undefined function: {}", name),
                        span: expr.1.clone(),
                    });
                }
            } else {
                // Attribute 등 다른 경우는 func_name 자체를 분석
                analyze_expr_function(func_name, scopes, ctx, locals, assigned)?;
            }
            for a in args {
                analyze_expr_function(a, scopes, ctx, locals, assigned)?;
            }
            Ok(())
        }
        Expr::Attribute { object, .. } => {
            analyze_expr_function(object, scopes, ctx, locals, assigned)?;
            Ok(())
        }
        Expr::List(elements) => {
            for elem in elements {
                analyze_expr_function(elem, scopes, ctx, locals, assigned)?;
            }
            Ok(())
        }
        Expr::Dict(pairs) => {
            for (key, value) in pairs {
                analyze_expr_function(key, scopes, ctx, locals, assigned)?;
                analyze_expr_function(value, scopes, ctx, locals, assigned)?;
            }
            Ok(())
        }
        Expr::Index { object, index } => {
            analyze_expr_function(object, scopes, ctx, locals, assigned)?;
            analyze_expr_function(index, scopes, ctx, locals, assigned)?;
            Ok(())
        }
        Expr::Lambda { params, body } => {
            // Check for unbound captured variables
            let mut free_vars = HashSet::new();
            collect_free_vars(body, params, &mut free_vars);

            for var in &free_vars {
                if locals.contains(var) && !assigned.contains(var) {
                    return Err(SemanticError {
                        message: format!("Unbound local variable '{}' captured by lambda", var),
                        span: expr.1.clone(),
                    });
                }
            }

            // Analyze the lambda body in a new scope
            scopes.push();
            for p in params {
                scopes.define(p.clone());
            }

            // The body of a lambda is an expression, so it can't contain assignments.
            // Its locals are just its parameters.
            let lambda_locals: HashSet<String> = params.iter().cloned().collect();
            analyze_expr_function(body, scopes, ctx, &lambda_locals, &lambda_locals)?;

            scopes.pop();
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::ast::{BinaryOp, Expr, Literal, Stmt};

    fn make_expr(expr: Expr) -> ExprS {
        (expr, 0..1)
    }

    fn make_stmt(stmt: Stmt) -> StmtS {
        (stmt, 0..1)
    }

    // ========== 스코프 해석 테스트 ==========

    #[test]
    fn test_analyze_global_variable() {
        let program = vec![
            make_stmt(Stmt::Assign {
                target: make_expr(Expr::Variable("x".to_string())),
                value: make_expr(Expr::Literal(Literal::Int(42))),
            }),
            make_stmt(Stmt::Expr(make_expr(Expr::Variable("x".to_string())))),
        ];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_undefined_variable() {
        let program = vec![make_stmt(Stmt::Expr(make_expr(Expr::Variable(
            "undefined".to_string(),
        ))))];

        let result = analyze(&program);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.message.contains("Undefined variable"));
        }
    }

    #[test]
    fn test_analyze_function_definition() {
        let program = vec![
            make_stmt(Stmt::Def {
                name: "foo".to_string(),
                params: vec![],
                body: vec![make_stmt(Stmt::Return(make_expr(Expr::Literal(
                    Literal::Int(42),
                ))))],
            }),
            make_stmt(Stmt::Expr(make_expr(Expr::Call {
                func_name: Box::new(make_expr(Expr::Variable("foo".to_string()))),
                args: vec![],
            }))),
        ];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_undefined_function() {
        let program = vec![make_stmt(Stmt::Expr(make_expr(Expr::Call {
            func_name: Box::new(make_expr(Expr::Variable("undefined".to_string()))),
            args: vec![],
        })))];

        let result = analyze(&program);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.message.contains("Undefined function"));
        }
    }

    #[test]
    fn test_analyze_function_parameters() {
        let program = vec![make_stmt(Stmt::Def {
            name: "add".to_string(),
            params: vec!["a".to_string(), "b".to_string()],
            body: vec![make_stmt(Stmt::Return(make_expr(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(make_expr(Expr::Variable("a".to_string()))),
                right: Box::new(make_expr(Expr::Variable("b".to_string()))),
            })))],
        })];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_unbound_local_variable() {
        // 함수 내에서 로컬 변수를 할당 전에 사용하는 경우
        let program = vec![make_stmt(Stmt::Def {
            name: "foo".to_string(),
            params: vec![],
            body: vec![
                make_stmt(Stmt::Expr(make_expr(Expr::Variable("x".to_string())))),
                make_stmt(Stmt::Assign {
                    target: make_expr(Expr::Variable("x".to_string())),
                    value: make_expr(Expr::Literal(Literal::Int(42))),
                }),
            ],
        })];

        let result = analyze(&program);
        assert!(result.is_err());
        if let Err(err) = result {
            assert!(err.message.contains("Unbound local variable"));
        }
    }

    #[test]
    fn test_analyze_builtin_function() {
        let program = vec![make_stmt(Stmt::Expr(make_expr(Expr::Call {
            func_name: Box::new(make_expr(Expr::Variable("print".to_string()))),
            args: vec![make_expr(Expr::Literal(Literal::Int(42)))],
        })))];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_nested_scopes() {
        let program = vec![
            make_stmt(Stmt::Assign {
                target: make_expr(Expr::Variable("x".to_string())),
                value: make_expr(Expr::Literal(Literal::Int(1))),
            }),
            make_stmt(Stmt::Def {
                name: "outer".to_string(),
                params: vec![],
                body: vec![make_stmt(Stmt::Def {
                    name: "inner".to_string(),
                    params: vec![],
                    body: vec![make_stmt(Stmt::Return(make_expr(Expr::Variable(
                        "x".to_string(),
                    ))))],
                })],
            }),
        ];

        let result = analyze(&program);
        // 글로벌 x를 inner 함수에서 참조 가능
        assert!(result.is_ok());
    }

    // ========== 타입 체킹 테스트 ==========

    #[test]
    fn test_analyze_type_error_add_bool() {
        let program = vec![make_stmt(Stmt::Expr(make_expr(Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(make_expr(Expr::Literal(Literal::Bool(true)))),
            right: Box::new(make_expr(Expr::Literal(Literal::Int(42)))),
        })))];

        let result = analyze(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_logical_on_int_allowed() {
        // Python allows int in logical operations (truthy/falsy)
        let program = vec![make_stmt(Stmt::Expr(make_expr(Expr::Binary {
            op: BinaryOp::And,
            left: Box::new(make_expr(Expr::Literal(Literal::Int(1)))),
            right: Box::new(make_expr(Expr::Literal(Literal::Int(2)))),
        })))];

        let result = analyze(&program);
        assert!(
            result.is_ok(),
            "Logical operations on int should be allowed"
        );
    }

    #[test]
    fn test_analyze_if_condition_type() {
        let program = vec![make_stmt(Stmt::If {
            condition: make_expr(Expr::Literal(Literal::Bool(true))),
            then_block: vec![make_stmt(Stmt::Expr(make_expr(Expr::Literal(
                Literal::Int(1),
            ))))],
            elif_blocks: vec![],
            else_block: None,
        })];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_while_condition_type() {
        let program = vec![make_stmt(Stmt::While {
            condition: make_expr(Expr::Literal(Literal::Bool(true))),
            body: vec![make_stmt(Stmt::Expr(make_expr(Expr::Literal(
                Literal::Int(1),
            ))))],
        })];

        let result = analyze(&program);
        assert!(result.is_ok());
    }

    #[test]
    fn test_analyze_return_type_consistency() {
        // 같은 함수에서 다른 타입을 반환하는 경우
        let program = vec![make_stmt(Stmt::Def {
            name: "foo".to_string(),
            params: vec!["x".to_string()],
            body: vec![make_stmt(Stmt::If {
                condition: make_expr(Expr::Variable("x".to_string())),
                then_block: vec![make_stmt(Stmt::Return(make_expr(Expr::Literal(
                    Literal::Int(1),
                ))))],
                elif_blocks: vec![],
                else_block: Some(vec![make_stmt(Stmt::Return(make_expr(Expr::Literal(
                    Literal::Bool(true),
                ))))]),
            })],
        })];

        let result = analyze(&program);
        assert!(result.is_err());
    }

    // ========== 에러 케이스 테스트 ==========

    #[test]
    fn test_analyze_multiple_errors() {
        // 여러 개의 에러가 있는 경우, 첫 번째 에러만 보고됨
        let program = vec![
            make_stmt(Stmt::Expr(make_expr(Expr::Variable(
                "undefined1".to_string(),
            )))),
            make_stmt(Stmt::Expr(make_expr(Expr::Variable(
                "undefined2".to_string(),
            )))),
        ];

        let result = analyze(&program);
        assert!(result.is_err());
    }

    #[test]
    fn test_analyze_complex_expression() {
        let program = vec![make_stmt(Stmt::Assign {
            target: make_expr(Expr::Variable("result".to_string())),
            value: make_expr(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(make_expr(Expr::Binary {
                    op: BinaryOp::Multiply,
                    left: Box::new(make_expr(Expr::Literal(Literal::Int(2)))),
                    right: Box::new(make_expr(Expr::Literal(Literal::Int(3)))),
                })),
                right: Box::new(make_expr(Expr::Literal(Literal::Int(4)))),
            }),
        })];

        let result = analyze(&program);
        assert!(result.is_ok());
    }
}
