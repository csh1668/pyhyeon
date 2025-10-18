use std::collections::{HashMap, HashSet};

use crate::parser::ast::{BinaryOp, Expr, ExprS, Stmt, StmtS, UnaryOp};

use super::{SemanticError, SemanticResult};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ty {
    Int,
    Bool,
    String,
    NoneType,
    Unknown,
}

#[derive(Default, Clone)]
struct TypeEnv {
    // Only module/function frames; no block scopes
    frames: Vec<HashMap<String, Ty>>,
}

impl TypeEnv {
    fn new() -> Self {
        Self {
            frames: vec![HashMap::new()],
        }
    }
    fn push(&mut self) {
        self.frames.push(HashMap::new());
    }
    fn pop(&mut self) {
        self.frames.pop();
    }
    fn get(&self, name: &str) -> Option<Ty> {
        for f in self.frames.iter().rev() {
            if let Some(t) = f.get(name) {
                return Some(*t);
            }
        }
        None
    }
    fn set(&mut self, name: String, ty: Ty) {
        if let Some(cur) = self.frames.last_mut() {
            cur.insert(name, ty);
        }
    }
}

pub fn typecheck_program(program: &[StmtS], ctx: &super::ProgramContext) -> SemanticResult<()> {
    let mut tenv = TypeEnv::new();
    // Optionally register globals that have known types beforehand

    // Walk module-level statements
    for stmt in program {
        tc_stmt(stmt, &mut tenv, ctx, None)?;
    }
    Ok(())
}

fn tc_stmt(
    stmt: &StmtS,
    tenv: &mut TypeEnv,
    ctx: &super::ProgramContext,
    current_fn_return: Option<*mut Ty>,
) -> SemanticResult<()> {
    match &stmt.0 {
        Stmt::Assign { name, value } => {
            let rhs = tc_expr(value, tenv, ctx)?;
            let prev = tenv.get(name);
            let new_ty = match (prev, rhs) {
                (None, t) => t,
                (Some(Ty::Unknown), t) => t,
                (Some(t), Ty::Unknown) => t,
                (Some(a), b) if a == b => a,
                (Some(a), b) => {
                    return Err(SemanticError {
                        message: format!(
                            "TypeError: cannot assign value of type {:?} to variable of type {:?}",
                            b, a
                        ),
                        span: value.1.clone(),
                    });
                }
            };
            tenv.set(name.clone(), new_ty);
            Ok(())
        }
        Stmt::Expr(expr) => {
            let _ = tc_expr(expr, tenv, ctx)?;
            Ok(())
        }
        Stmt::Return(expr) => {
            let t = tc_expr(expr, tenv, ctx)?;
            if let Some(ptr) = current_fn_return {
                // SAFETY: ptr was created from a valid &mut Ty in Def arm and lives until function scope ends
                unsafe {
                    let old = *ptr;
                    let new = unify_return(old, t).ok_or_else(|| SemanticError {
                        message: format!(
                            "TypeError: inconsistent return types in function: {:?} vs {:?}",
                            old, t
                        ),
                        span: expr.1.clone(),
                    })?;
                    *ptr = new;
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
            ensure_bool(condition, tenv, ctx)?;
            // Base snapshot (environment before entering branches)
            let base = snapshot_env(tenv);

            // then branch env and assigned set
            let mut then_env = snapshot_env(&base);
            let mut then_assigned: HashSet<String> = HashSet::new();
            with_env(&mut then_env, |e| {
                for s in then_block {
                    let _ = tc_stmt(s, e, ctx, current_fn_return);
                }
            });
            collect_assigned(&base, &then_env, &mut then_assigned);

            // collect elif branches
            let mut branches: Vec<(TypeEnv, HashSet<String>)> = Vec::new();
            branches.push((then_env, then_assigned));
            for (cond, block) in elif_blocks {
                ensure_bool(cond, tenv, ctx)?;
                let mut env_i = snapshot_env(&base);
                with_env(&mut env_i, |e| {
                    for s in block {
                        let _ = tc_stmt(s, e, ctx, current_fn_return);
                    }
                });
                let mut assigned_i = HashSet::new();
                collect_assigned(&base, &env_i, &mut assigned_i);
                branches.push((env_i, assigned_i));
            }

            // optional else branch
            if let Some(block) = else_block {
                let mut else_env = snapshot_env(&base);
                let mut else_assigned: HashSet<String> = HashSet::new();
                with_env(&mut else_env, |e| {
                    for s in block {
                        let _ = tc_stmt(s, e, ctx, current_fn_return);
                    }
                });
                collect_assigned(&base, &else_env, &mut else_assigned);
                branches.push((else_env, else_assigned));
            }

            // union of assigned vars across all present branches
            let mut all_assigned: HashSet<String> = HashSet::new();
            for (_, a) in &branches {
                all_assigned.extend(a.iter().cloned());
            }

            // merge rule: only compare types among branches where the var is assigned;
            // ignore Unknown; if multiple concrete types disagree -> error; otherwise commit the concrete type if any
            for var in all_assigned {
                let mut merged: Option<Ty> = None;
                for (env_b, assigned_b) in &branches {
                    if assigned_b.contains(&var) {
                        let t_b = get_var_type(env_b, &var);
                        if t_b != Ty::Unknown {
                            if let Some(m) = merged {
                                if m != t_b {
                                    return Err(SemanticError {
                                        message: format!(
                                            "TypeError: variable '{}' has incompatible types across branches",
                                            var
                                        ),
                                        span: condition.1.clone(),
                                    });
                                }
                            } else {
                                merged = Some(t_b);
                            }
                        }
                    }
                }
                if let Some(t) = merged {
                    tenv.set(var, t);
                }
            }

            Ok(())
        }
        Stmt::While { condition, body } => {
            ensure_bool(condition, tenv, ctx)?;
            // Conservative: do not commit new types from loop body to outer env
            let mut loop_env = snapshot_env(tenv);
            with_env(&mut loop_env, |e| {
                for s in body {
                    let _ = tc_stmt(s, e, ctx, current_fn_return);
                }
            });
            Ok(())
        }
        Stmt::Def {
            name: _,
            params,
            body,
        } => {
            // Type-check function body in its own frame
            tenv.push();
            for p in params {
                tenv.set(p.clone(), Ty::Unknown);
            }
            let mut ret: Ty = Ty::Unknown;
            let ret_ptr: *mut Ty = &mut ret;
            for s in body {
                tc_stmt(s, tenv, ctx, Some(ret_ptr))?;
            }
            tenv.pop();
            let _ = ret; // currently unused for call-site checks
            Ok(())
        }
    }
}

fn tc_expr(expr: &ExprS, tenv: &mut TypeEnv, ctx: &super::ProgramContext) -> SemanticResult<Ty> {
    match &expr.0 {
        Expr::Literal(lit) => Ok(match lit {
            crate::parser::ast::Literal::Bool(_) => Ty::Bool,
            crate::parser::ast::Literal::Int(_) => Ty::Int,
            crate::parser::ast::Literal::String(_) => Ty::String,
            crate::parser::ast::Literal::None => Ty::NoneType,
        }),
        Expr::Variable(name) => Ok(tenv.get(name).unwrap_or(Ty::Unknown)),
        Expr::Unary { op, expr: inner } => {
            let t = tc_expr(inner, tenv, ctx)?;
            match op {
                UnaryOp::Not => expect_bool(t, expr.1.clone()),
                UnaryOp::Negate | UnaryOp::Pos => expect_int(t, expr.1.clone()),
            }
        }
        Expr::Binary { op, left, right } => {
            let tl = tc_expr(left, tenv, ctx)?;
            let tr = tc_expr(right, tenv, ctx)?;
            match op {
                BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::FloorDivide
                | BinaryOp::Modulo => expect_int_pair(tl, tr, expr.1.clone()).map(|_| Ty::Int),
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => {
                    expect_int_pair(tl, tr, expr.1.clone()).map(|_| Ty::Bool)
                }
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    expect_same_or_unknown(tl, tr, expr.1.clone()).map(|_| Ty::Bool)
                }
                BinaryOp::And | BinaryOp::Or => Ok(Ty::Bool),
            }
        }
        Expr::Call { func_name, args } => {
            if let Some(bi) = crate::builtins::lookup(func_name) {
                if bi.arity() != args.len() {
                    return Err(SemanticError {
                        message: format!(
                            "ArityError: {} takes {} positional argument(s) but {} given",
                            bi.name,
                            bi.arity(),
                            args.len()
                        ),
                        span: expr.1.clone(),
                    });
                }
                match bi.name {
                    "print" => {
                        let _ = tc_expr(&args[0], tenv, ctx)?;
                        Ok(Ty::NoneType)
                    }
                    "input" => Ok(Ty::Int),
                    "int" => {
                        let _ = tc_expr(&args[0], tenv, ctx)?;
                        Ok(Ty::Int)
                    }
                    "bool" => {
                        let _ = tc_expr(&args[0], tenv, ctx)?;
                        Ok(Ty::Bool)
                    }
                    _ => Ok(Ty::Unknown),
                }
            } else if let Some(expected) = ctx.functions.get(func_name) {
                if *expected != args.len() {
                    return Err(SemanticError {
                        message: format!(
                            "ArityError: function '{}' takes {} positional arguments but {} were given",
                            func_name,
                            expected,
                            args.len()
                        ),
                        span: expr.1.clone(),
                    });
                }
                for a in args {
                    let _ = tc_expr(a, tenv, ctx)?;
                }
                Ok(Ty::Unknown)
            } else {
                Err(SemanticError {
                    message: format!("Undefined function: {}", func_name),
                    span: expr.1.clone(),
                })
            }
        }
    }
}

fn ensure_bool(
    cond: &ExprS,
    tenv: &mut TypeEnv,
    ctx: &super::ProgramContext,
) -> SemanticResult<()> {
    let t = tc_expr(cond, tenv, ctx)?;
    if t != Ty::Bool {
        return Err(SemanticError {
            message: "TypeError: condition must be Bool".to_string(),
            span: cond.1.clone(),
        });
    }
    Ok(())
}

fn expect_int(t: Ty, span: crate::types::Span) -> SemanticResult<Ty> {
    match t {
        Ty::Int => Ok(Ty::Int),
        Ty::Unknown => Ok(Ty::Int), // optimistic
        _ => Err(SemanticError {
            message: format!("TypeError: expected Int, got {:?}", t),
            span,
        }),
    }
}

fn expect_bool(t: Ty, span: crate::types::Span) -> SemanticResult<Ty> {
    match t {
        Ty::Bool => Ok(Ty::Bool),
        Ty::Unknown => Ok(Ty::Bool), // optimistic
        _ => Err(SemanticError {
            message: format!("TypeError: expected Bool, got {:?}", t),
            span,
        }),
    }
}

fn expect_int_pair(t1: Ty, t2: Ty, span: crate::types::Span) -> SemanticResult<()> {
    match (t1, t2) {
        (Ty::Int, Ty::Int) => Ok(()),
        (Ty::Unknown, Ty::Int) | (Ty::Int, Ty::Unknown) | (Ty::Unknown, Ty::Unknown) => Ok(()),
        _ => Err(SemanticError {
            message: format!("TypeError: expected Int and Int, got {:?} and {:?}", t1, t2),
            span,
        }),
    }
}

// fn expect_bool_pair(t1: Ty, t2: Ty, span: crate::types::Span) -> SemanticResult<()> {
//     match (t1, t2) {
//         (Ty::Bool, Ty::Bool) => Ok(()),
//         (Ty::Unknown, Ty::Bool) | (Ty::Bool, Ty::Unknown) | (Ty::Unknown, Ty::Unknown) => Ok(()),
//         _ => Err(SemanticError {
//             message: format!(
//                 "TypeError: expected Bool and Bool, got {:?} and {:?}",
//                 t1, t2
//             ),
//             span,
//         }),
//     }
// }

fn expect_same_or_unknown(t1: Ty, t2: Ty, span: crate::types::Span) -> SemanticResult<()> {
    if t1 == Ty::Unknown || t2 == Ty::Unknown || t1 == t2 {
        Ok(())
    } else {
        Err(SemanticError {
            message: format!(
                "TypeError: equality operands must have same type, got {:?} and {:?}",
                t1, t2
            ),
            span,
        })
    }
}

fn unify_return(a: Ty, b: Ty) -> Option<Ty> {
    match (a, b) {
        (Ty::Unknown, x) | (x, Ty::Unknown) => Some(x),
        (x, y) if x == y => Some(x),
        _ => None,
    }
}

fn snapshot_env(tenv: &TypeEnv) -> TypeEnv {
    TypeEnv {
        frames: tenv.frames.clone(),
    }
}
fn with_env<F: FnOnce(&mut TypeEnv)>(tenv: &mut TypeEnv, f: F) {
    f(tenv);
}
fn collect_assigned(base: &TypeEnv, changed: &TypeEnv, out: &mut HashSet<String>) {
    let base_top = base.frames.last().unwrap();
    let changed_top = changed.frames.last().unwrap();
    for (k, v) in changed_top.iter() {
        if base_top.get(k) != Some(v) {
            out.insert(k.clone());
        }
    }
}
fn get_var_type(env: &TypeEnv, var: &str) -> Ty {
    env.get(var).unwrap_or(Ty::Unknown)
}
