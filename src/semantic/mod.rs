pub mod scope;
pub mod typecheck;

use crate::parser::ast::{Expr, Stmt};

#[derive(Debug)]
pub struct SemanticError {
    pub message: String,
}

pub type SemanticResult<T> = Result<T, SemanticError>;

pub fn analyze(program: &[Stmt]) -> SemanticResult<()> {
    // Skeleton: wire up passes here (scope resolution → type checks)
    let mut scope = scope::ScopeStack::new();
    for stmt in program {
        analyze_stmt(stmt, &mut scope)?;
    }
    Ok(())
}

fn analyze_stmt(stmt: &Stmt, scopes: &mut scope::ScopeStack) -> SemanticResult<()> {
    match stmt {
        Stmt::Def { name, params, body } => {
            scopes.define(name.clone());
            scopes.push();
            for p in params {
                scopes.define(p.clone());
            }
            for s in body {
                analyze_stmt(s, scopes)?;
            }
            scopes.pop();
            Ok(())
        }
        Stmt::Assign { name, value } => {
            analyze_expr(value, scopes)?;
            if !scopes.is_defined(name) {
                scopes.define(name.clone());
            }
            Ok(())
        }
        Stmt::If {
            condition,
            then_block,
            elif_blocks,
            else_block,
        } => {
            analyze_expr(condition, scopes)?;
            scopes.push();
            for s in then_block {
                analyze_stmt(s, scopes)?;
            }
            scopes.pop();
            for (cond, block) in elif_blocks {
                analyze_expr(cond, scopes)?;
                scopes.push();
                for s in block {
                    analyze_stmt(s, scopes)?;
                }
                scopes.pop();
            }
            if let Some(block) = else_block {
                scopes.push();
                for s in block {
                    analyze_stmt(s, scopes)?;
                }
                scopes.pop();
            }
            Ok(())
        }
        Stmt::While { condition, body } => {
            analyze_expr(condition, scopes)?;
            scopes.push();
            for s in body {
                analyze_stmt(s, scopes)?;
            }
            scopes.pop();
            Ok(())
        }
        Stmt::Return(expr) => analyze_expr(expr, scopes),
        Stmt::Expr(expr) => analyze_expr(expr, scopes),
    }
}

fn analyze_expr(expr: &Expr, scopes: &mut scope::ScopeStack) -> SemanticResult<()> {
    match expr {
        Expr::Literal(_) => Ok(()),
        Expr::Variable(name) => {
            if !scopes.is_defined(name) {
                return Err(SemanticError {
                    message: format!("Undefined variable: {}", name),
                });
            }
            Ok(())
        }
        Expr::Unary { op: _, expr } => analyze_expr(expr, scopes),
        Expr::Binary { op: _, left, right } => {
            analyze_expr(left, scopes)?;
            analyze_expr(right, scopes)
        }
        Expr::Call { func_name: _, args } => {
            for a in args {
                analyze_expr(a, scopes)?;
            }
            Ok(())
        }
    }
}
