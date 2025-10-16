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
pub(crate) struct ProgramContext {
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
    // 1) 이름 해석(스코프) + 간단 규칙 확인
    let mut ctx = ProgramContext::new_with_builtins();
    let mut scopes = scope::ScopeStack::new();
    // preload builtins into global scope for resolution
    for b in ctx.builtins.clone() { scopes.define(b); }

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
        Stmt::Def { name, params, body } => {
            // 정의는 현재 스코프(모듈)에 바인딩
            scopes.define(name.clone());
            ctx.functions.insert(name.clone(), params.len());
            analyze_function(name, params, body, scopes, ctx, stmt.1.clone())
        }
        Stmt::Assign { name, value } => {
            analyze_expr_module(value, scopes, ctx)?;
            if !scopes.is_defined(name) { scopes.define(name.clone()); }
            Ok(())
        }
        Stmt::If { condition, then_block, elif_blocks, else_block } => {
            analyze_expr_module(condition, scopes, ctx)?;
            // 블록 스코프는 만들지 않는다
            for s in then_block { analyze_stmt_module(s, scopes, ctx)?; }
            for (cond, block) in elif_blocks {
                analyze_expr_module(cond, scopes, ctx)?;
                for s in block { analyze_stmt_module(s, scopes, ctx)?; }
            }
            if let Some(block) = else_block {
                for s in block { analyze_stmt_module(s, scopes, ctx)?; }
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            analyze_expr_module(condition, scopes, ctx)?;
            for s in body { analyze_stmt_module(s, scopes, ctx)?; }
            Ok(())
        }
        Stmt::Return(expr) => analyze_expr_module(expr, scopes, ctx),
        Stmt::Expr(expr) => analyze_expr_module(expr, scopes, ctx),
    }
}

fn analyze_expr_module(expr: &ExprS, scopes: &mut scope::ScopeStack, ctx: &ProgramContext) -> SemanticResult<()> {
    match &expr.0 {
        Expr::Literal(_) => Ok(()),
        Expr::Variable(name) => {
            if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                return Err(SemanticError { message: format!("Undefined variable: {}", name), span: expr.1.clone() });
            }
            Ok(())
        }
        Expr::Unary { op: _, expr: inner } => analyze_expr_module(inner, scopes, ctx),
        Expr::Binary { op: _, left, right } => {
            analyze_expr_module(left, scopes, ctx)?;
            analyze_expr_module(right, scopes, ctx)
        }
        Expr::Call { func_name, args } => {
            if !scopes.is_defined(func_name) && !ctx.is_builtin(func_name) {
                return Err(SemanticError { message: format!("Undefined function: {}", func_name), span: expr.1.clone() });
            }
            for a in args { analyze_expr_module(a, scopes, ctx)?; }
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
    for p in params { scopes.define(p.clone()); }

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
            Stmt::Assign { name, .. } => { locals.insert(name.clone()); }
            Stmt::Def { name, .. } => { locals.insert(name.clone()); }
            Stmt::If { then_block, elif_blocks, else_block, .. } => {
                collect_locals(then_block, locals);
                for (_, block) in elif_blocks { collect_locals(block, locals); }
                if let Some(b) = else_block { collect_locals(b, locals); }
            }
            Stmt::While { body, .. } => { collect_locals(body, locals); }
            Stmt::Return(_) | Stmt::Expr(_) => {}
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
        Stmt::Assign { name, value } => {
            analyze_expr_function(value, scopes, ctx, locals, assigned)?;
            assigned.insert(name.clone());
            if !scopes.is_defined(name) { scopes.define(name.clone()); }
            Ok(())
        }
        Stmt::Def { name, params, body } => {
            // 함수 정의도 로컬에 바인딩
            if !scopes.is_defined(name) { scopes.define(name.clone()); }
            // 중첩 함수: 캡처 미지원 → 내부에서 바깥 로컬 참조 시 이후 타입/이름 단계에서 오류가 날 수 있음
            let mut inner_ctx = ProgramContext { builtins: ctx.builtins.clone(), functions: ctx.functions.clone() };
            inner_ctx.functions.insert(name.clone(), params.len());
            analyze_function(name, params, body, scopes, &mut inner_ctx, stmt.1.clone())
        }
        Stmt::If { condition, then_block, elif_blocks, else_block } => {
            analyze_expr_function(condition, scopes, ctx, locals, assigned)?;
            for s in then_block { analyze_stmt_function(s, scopes, ctx, locals, assigned)?; }
            for (cond, block) in elif_blocks {
                analyze_expr_function(cond, scopes, ctx, locals, assigned)?;
                for s in block { analyze_stmt_function(s, scopes, ctx, locals, assigned)?; }
            }
            if let Some(block) = else_block {
                for s in block { analyze_stmt_function(s, scopes, ctx, locals, assigned)?; }
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            analyze_expr_function(condition, scopes, ctx, locals, assigned)?;
            for s in body { analyze_stmt_function(s, scopes, ctx, locals, assigned)?; }
            Ok(())
        }
        Stmt::Return(expr) => analyze_expr_function(expr, scopes, ctx, locals, assigned),
        Stmt::Expr(expr) => analyze_expr_function(expr, scopes, ctx, locals, assigned),
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
                    return Err(SemanticError { message: format!("Unbound local variable: {}", name), span: expr.1.clone() });
                }
                return Ok(());
            }
            if !scopes.is_defined(name) && !ctx.is_builtin(name) {
                return Err(SemanticError { message: format!("Undefined variable: {}", name), span: expr.1.clone() });
            }
            Ok(())
        }
        Expr::Unary { op: _, expr: inner } => analyze_expr_function(inner, scopes, ctx, locals, assigned),
        Expr::Binary { op: _, left, right } => {
            analyze_expr_function(left, scopes, ctx, locals, assigned)?;
            analyze_expr_function(right, scopes, ctx, locals, assigned)
        }
        Expr::Call { func_name, args } => {
            if locals.contains(func_name) {
                if !assigned.contains(func_name) {
                    return Err(SemanticError { message: format!("Unbound local function: {}", func_name), span: expr.1.clone() });
                }
            } else if !scopes.is_defined(func_name) && !ctx.is_builtin(func_name) {
                return Err(SemanticError { message: format!("Undefined function: {}", func_name), span: expr.1.clone() });
            }
            for a in args { analyze_expr_function(a, scopes, ctx, locals, assigned)?; }
            Ok(())
        }
    }
}
