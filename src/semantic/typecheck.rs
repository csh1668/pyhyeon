use std::collections::{HashMap, HashSet};

use crate::parser::ast::{BinaryOp, Expr, ExprS, Stmt, StmtS, UnaryOp};

use super::{SemanticError, SemanticResult};

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    Int,
    Bool,
    String,
    Float,
    NoneType,
    Unknown,
    List(Box<Ty>),
    Dict(Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    Range,
    Function,
    MapIter(Box<Ty>),
    FilterIter(Box<Ty>),
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
                return Some(t.clone());
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
        tc_stmt(stmt, &mut tenv, ctx, &mut Some(Ty::Unknown), false)?;
    }
    Ok(())
}

fn tc_stmt(
    stmt: &StmtS,
    tenv: &mut TypeEnv,
    ctx: &super::ProgramContext,
    current_fn_return: &mut Option<Ty>,
    in_loop: bool,
) -> SemanticResult<()> {
    match &stmt.0 {
        Stmt::Break => {
            if !in_loop {
                return Err(SemanticError {
                    message: "SyntaxError: 'break' outside loop".to_string(),
                    span: stmt.1.clone(),
                });
            }
            Ok(())
        }
        Stmt::Continue => {
            if !in_loop {
                return Err(SemanticError {
                    message: "SyntaxError: 'continue' outside loop".to_string(),
                    span: stmt.1.clone(),
                });
            }
            Ok(())
        }
        Stmt::Pass => {
            // Pass는 항상 허용 (no-op)
            Ok(())
        }
        Stmt::Class { .. } => {
            // 클래스 정의는 타입 체크 스킵 (v1에서는 간단히 처리)
            Ok(())
        }
        Stmt::Assign { target, value } => {
            let rhs = tc_expr(value, tenv, ctx)?;
            // target이 Variable이면 타입 추적, Attribute이면 검증만
            match &target.0 {
                Expr::Variable(name) => {
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
                }
                Expr::Attribute { .. } => {
                    // Attribute 할당은 타입 추적하지 않음
                    let _ = tc_expr(target, tenv, ctx)?;
                }
                Expr::Index { .. } => {
                    // Index 할당은 타입 추적하지 않음
                    let _ = tc_expr(target, tenv, ctx)?;
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
        Stmt::Expr(expr) => {
            let _ = tc_expr(expr, tenv, ctx)?;
            Ok(())
        }
        Stmt::Return(expr) => {
            let t = tc_expr(expr, tenv, ctx)?;
            if let Some(ptr) = current_fn_return {
                let old = ptr.clone();
                let new = unify_return(old.clone(), t.clone()).ok_or_else(|| SemanticError {
                    message: format!(
                        "TypeError: inconsistent return types in function: {:?} vs {:?}",
                        old, t
                    ),
                    span: expr.1.clone(),
                })?;
                *ptr = new;
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
                    let _ = tc_stmt(s, e, ctx, current_fn_return, in_loop);
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
                        let _ = tc_stmt(s, e, ctx, current_fn_return, in_loop);
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
                        let _ = tc_stmt(s, e, ctx, current_fn_return, in_loop);
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
                            if let Some(ref m) = merged {
                                if *m != t_b {
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
                    let _ = tc_stmt(s, e, ctx, current_fn_return, true);
                }
            });
            Ok(())
        }
        Stmt::For {
            var,
            iterable,
            body,
        } => {
            let iterable_ty = tc_expr(iterable, tenv, ctx)?;
            let loop_var_ty = match &iterable_ty {
                Ty::Range => Ty::Int,
                Ty::List(elem_ty) => *elem_ty.clone(),
                Ty::Dict(key_ty, _) => *key_ty.clone(),
                Ty::String => Ty::String,
                Ty::Unknown => Ty::Unknown,
                _ => {
                    return Err(SemanticError {
                        message: format!("TypeError: type '{:?}' is not iterable", iterable_ty),
                        span: iterable.1.clone(),
                    });
                }
            };

            let mut loop_env = snapshot_env(tenv);
            with_env(&mut loop_env, |e| {
                e.set(var.clone(), loop_var_ty);
                for s in body {
                    let _ = tc_stmt(s, e, ctx, current_fn_return, true);
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
            let mut fn_return_ty = Some(Ty::Unknown);
            for s in body {
                tc_stmt(s, tenv, ctx, &mut fn_return_ty, false)?;
            }
            tenv.pop();
            let _ = fn_return_ty; // currently unused for call-site checks
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
            crate::parser::ast::Literal::Float(_) => Ty::Float,
            crate::parser::ast::Literal::None => Ty::NoneType,
        }),
        Expr::Variable(name) => Ok(tenv.get(name).unwrap_or(Ty::Unknown)),
        Expr::Unary { op, expr: inner } => {
            let t = tc_expr(inner, tenv, ctx)?;
            match op {
                UnaryOp::Not => expect_bool(t, expr.1.clone()),
                UnaryOp::Negate | UnaryOp::Pos => expect_int_or_float(t, expr.1.clone()),
            }
        }
        Expr::Binary { op, left, right } => {
            let tl = tc_expr(left, tenv, ctx)?;
            let tr = tc_expr(right, tenv, ctx)?;
            match op {
                BinaryOp::Add => {
                    // Type rules for addition:
                    // - Int + Int -> Int
                    // - Float + Float -> Float
                    // - Int + Float -> Float (promotion)
                    // - String + String -> String
                    // - Operations with Unknown are optimistic.
                    match (tl, tr) {
                        (Ty::Int, Ty::Int) => Ok(Ty::Int),
                        (Ty::Float, Ty::Float) => Ok(Ty::Float),
                        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ok(Ty::Float),
                        (Ty::String, Ty::String) => Ok(Ty::String),
                        (Ty::Unknown, Ty::Int) | (Ty::Int, Ty::Unknown) => Ok(Ty::Int),
                        (Ty::Unknown, Ty::Float) | (Ty::Float, Ty::Unknown) => Ok(Ty::Float),
                        (Ty::Unknown, Ty::String) | (Ty::String, Ty::Unknown) => Ok(Ty::String),
                        (Ty::Unknown, Ty::Unknown) => Ok(Ty::Unknown),
                        (tl, tr) => Err(SemanticError {
                            message: format!(
                                "TypeError: unsupported operand types for +: {:?} and {:?}",
                                tl, tr
                            ),
                            span: expr.1.clone(),
                        }),
                    }
                }
                BinaryOp::Multiply => {
                    // Type rules for multiplication:
                    // - Int * Int -> Int
                    // - Float * Float -> Float
                    // - Int * Float -> Float (promotion)
                    // - String * Int -> String
                    // - Operations with Unknown are optimistic.
                    match (tl, tr) {
                        (Ty::Int, Ty::Int) => Ok(Ty::Int),
                        (Ty::Float, Ty::Float) => Ok(Ty::Float),
                        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ok(Ty::Float),
                        (Ty::String, Ty::Int) | (Ty::Int, Ty::String) => Ok(Ty::String),
                        (Ty::Unknown, Ty::Int) | (Ty::Int, Ty::Unknown) => Ok(Ty::Int),
                        (Ty::Unknown, Ty::Float) | (Ty::Float, Ty::Unknown) => Ok(Ty::Float),
                        (Ty::Unknown, Ty::String) | (Ty::String, Ty::Unknown) => Ok(Ty::String),
                        (Ty::Unknown, Ty::Unknown) => Ok(Ty::Unknown),
                        (tl, tr) => Err(SemanticError {
                            message: format!(
                                "TypeError: unsupported operand types for *: {:?} and {:?}",
                                tl, tr
                            ),
                            span: expr.1.clone(),
                        }),
                    }
                }
                BinaryOp::Subtract | BinaryOp::Modulo => {
                    expect_numeric_pair(tl, tr, expr.1.clone())
                }
                BinaryOp::Divide => {
                    // / 연산은 항상 Float 반환 (Python 3 스타일)
                    expect_numeric_pair(tl, tr, expr.1.clone()).map(|_| Ty::Float)
                }
                BinaryOp::FloorDivide => {
                    // // 연산은 피연산자 타입 유지
                    expect_numeric_pair(tl, tr, expr.1.clone())
                }
                BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual => match (tl, tr) {
                    (Ty::Int, Ty::Int) => Ok(Ty::Bool),
                    (Ty::Float, Ty::Float) => Ok(Ty::Bool),
                    (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ok(Ty::Bool),
                    (Ty::String, Ty::String) => Ok(Ty::Bool),
                    (Ty::Unknown, Ty::Int) | (Ty::Int, Ty::Unknown) => Ok(Ty::Bool),
                    (Ty::Unknown, Ty::Float) | (Ty::Float, Ty::Unknown) => Ok(Ty::Bool),
                    (Ty::Unknown, Ty::String) | (Ty::String, Ty::Unknown) => Ok(Ty::Bool),
                    (Ty::Unknown, Ty::Unknown) => Ok(Ty::Bool),
                    (tl, tr) => Err(SemanticError {
                        message: format!("TypeError: cannot compare {:?} and {:?}", tl, tr),
                        span: expr.1.clone(),
                    }),
                },
                BinaryOp::Equal | BinaryOp::NotEqual => {
                    expect_same_or_unknown(tl, tr, expr.1.clone()).map(|_| Ty::Bool)
                }
                BinaryOp::And | BinaryOp::Or => Ok(Ty::Bool),
            }
        }
        Expr::Call { func_name, args } => {
            // func_name이 Variable인 경우에만 builtin 체크
            let func_name_str = if let Expr::Variable(name) = &func_name.0 {
                Some(name.as_str())
            } else {
                None
            };

            if let Some(name) = func_name_str {
                if let Some(bi) = crate::builtins::lookup(name) {
                    // Unified arity checking using Arity enum
                    if !bi.check_arity(args.len()) {
                        let msg = format!(
                            "ArityError: {}() takes {} argument(s) but {} given",
                            bi.name,
                            bi.arity.description(),
                            args.len()
                        );
                        return Err(SemanticError {
                            message: msg,
                            span: expr.1.clone(),
                        });
                    }

                    // Type-specific validation (type checking, not arity)
                    match bi.name {
                        "print" => {
                            // Type-check all arguments
                            for arg in args {
                                let _ = tc_expr(arg, tenv, ctx)?;
                            }
                            return Ok(Ty::NoneType);
                        }
                        "input" => {
                            // input() or input(prompt)
                            if args.len() == 1 {
                                let arg_ty = tc_expr(&args[0], tenv, ctx)?;
                                if arg_ty != Ty::String && arg_ty != Ty::Unknown {
                                    return Err(SemanticError {
                                        message: format!(
                                            "TypeError: input() prompt must be a string, got {:?}",
                                            arg_ty
                                        ),
                                        span: expr.1.clone(),
                                    });
                                }
                            }
                            return Ok(Ty::String);
                        }
                        "int" => {
                            let _ = tc_expr(&args[0], tenv, ctx)?;
                            return Ok(Ty::Int);
                        }
                        "float" => {
                            let _ = tc_expr(&args[0], tenv, ctx)?;
                            return Ok(Ty::Float);
                        }
                        "bool" => {
                            let _ = tc_expr(&args[0], tenv, ctx)?;
                            return Ok(Ty::Bool);
                        }
                        "str" => {
                            let _ = tc_expr(&args[0], tenv, ctx)?;
                            return Ok(Ty::String);
                        }
                        "len" => {
                            let arg_ty = tc_expr(&args[0], tenv, ctx)?;
                            return match arg_ty {
                                Ty::String | Ty::List(_) | Ty::Dict(_, _) => Ok(Ty::Int),
                                Ty::Unknown => Ok(Ty::Int),
                                _ => Err(SemanticError {
                                    message: format!(
                                        "TypeError: object of type {:?} has no len()",
                                        arg_ty
                                    ),
                                    span: expr.1.clone(),
                                }),
                            };
                        }
                        "range" => {
                            // range(stop) or range(start, stop) or range(start, stop, step)
                            // 모든 인자는 Int여야 함
                            for arg in args {
                                let arg_ty = tc_expr(arg, tenv, ctx)?;
                                if arg_ty != Ty::Int && arg_ty != Ty::Unknown {
                                    return Err(SemanticError {
                                        message: format!(
                                            "TypeError: range() arguments must be integers, got {:?}",
                                            arg_ty
                                        ),
                                        span: expr.1.clone(),
                                    });
                                }
                            }
                            // range는 iterator 객체를 반환 (타입 시스템에서는 Range로 처리)
                            return Ok(Ty::Range);
                        }
                        "assert" => {
                            let _ = tc_expr(&args[0], tenv, ctx)?;
                            return Ok(Ty::NoneType);
                        }
                        _ => {
                            // Generic fallback: type-check all arguments
                            for arg in args {
                                let _ = tc_expr(arg, tenv, ctx)?;
                            }
                            return Ok(Ty::Unknown);
                        }
                    }
                } else if let Some(&arity) = ctx.functions.get(name) {
                    // user-defined function
                    if args.len() != arity {
                        return Err(SemanticError {
                            message: format!(
                                "ArityError: function '{}' takes {} positional arguments but {} were given",
                                name,
                                arity,
                                args.len()
                            ),
                            span: expr.1.clone(),
                        });
                    }
                    for a in args {
                        let _ = tc_expr(a, tenv, ctx)?;
                    }
                    return Ok(Ty::Unknown);
                } else {
                    // Undefined function - semantic analysis should have caught this
                    // But we allow it in typecheck for flexibility
                    for a in args {
                        let _ = tc_expr(a, tenv, ctx)?;
                    }
                    return Ok(Ty::Unknown);
                }
            }

            // Attribute 등 다른 경우: 간단히 Unknown 반환
            let _ = tc_expr(func_name, tenv, ctx)?;
            for a in args {
                let _ = tc_expr(a, tenv, ctx)?;
            }
            Ok(Ty::Unknown)
        }
        Expr::Attribute { object, .. } => {
            let _ = tc_expr(object, tenv, ctx)?;
            Ok(Ty::Unknown) // Attribute는 Unknown 타입으로 처리
        }
        Expr::List(elements) => {
            let mut elem_ty = Ty::Unknown;
            for elem in elements {
                let ty = tc_expr(elem, tenv, ctx)?;
                if elem_ty == Ty::Unknown {
                    elem_ty = ty;
                } else if elem_ty != ty {
                    elem_ty = Ty::Unknown;
                    break;
                }
            }
            Ok(Ty::List(Box::new(elem_ty)))
        }
        Expr::Dict(pairs) => {
            let mut key_ty = Ty::Unknown;
            let mut val_ty = Ty::Unknown;
            for (key, value) in pairs {
                let k_ty = tc_expr(key, tenv, ctx)?;
                let v_ty = tc_expr(value, tenv, ctx)?;
                if key_ty == Ty::Unknown {
                    key_ty = k_ty;
                } else if key_ty != k_ty {
                    key_ty = Ty::Unknown;
                }
                if val_ty == Ty::Unknown {
                    val_ty = v_ty;
                } else if val_ty != v_ty {
                    val_ty = Ty::Unknown;
                }
                if key_ty == Ty::Unknown && val_ty == Ty::Unknown {
                    break;
                }
            }
            Ok(Ty::Dict(Box::new(key_ty), Box::new(val_ty)))
        }
        Expr::Index { object, index } => {
            let obj_ty = tc_expr(object, tenv, ctx)?;
            let idx_ty = tc_expr(index, tenv, ctx)?;
            match obj_ty {
                Ty::List(elem_ty) => {
                    if idx_ty != Ty::Int && idx_ty != Ty::Unknown {
                        return Err(SemanticError {
                            message: format!(
                                "TypeError: list indices must be integers, not {:?}",
                                idx_ty
                            ),
                            span: index.1.clone(),
                        });
                    }
                    Ok(*elem_ty)
                }
                Ty::Dict(key_ty, val_ty) => {
                    if idx_ty != *key_ty && idx_ty != Ty::Unknown && *key_ty != Ty::Unknown {
                        return Err(SemanticError {
                            message: format!(
                                "TypeError: dictionary key type mismatch: expected {:?}, got {:?}",
                                *key_ty, idx_ty
                            ),
                            span: index.1.clone(),
                        });
                    }
                    Ok(*val_ty)
                }
                Ty::String => {
                    if idx_ty != Ty::Int && idx_ty != Ty::Unknown {
                        return Err(SemanticError {
                            message: format!(
                                "TypeError: string indices must be integers, not {:?}",
                                idx_ty
                            ),
                            span: index.1.clone(),
                        });
                    }
                    Ok(Ty::String)
                }
                Ty::Unknown => Ok(Ty::Unknown),
                _ => Err(SemanticError {
                    message: format!("TypeError: type '{:?}' is not subscriptable", obj_ty),
                    span: object.1.clone(),
                }),
            }
        }
        Expr::Lambda { .. } => {
            // A full implementation would check the body and infer a more specific
            // function type, but for now, just marking it as a function is enough.
            Ok(Ty::Function)
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

fn expect_int_or_float(t: Ty, span: crate::types::Span) -> SemanticResult<Ty> {
    match t {
        Ty::Int => Ok(Ty::Int),
        Ty::Float => Ok(Ty::Float),
        Ty::Unknown => Ok(Ty::Unknown), // optimistic
        _ => Err(SemanticError {
            message: format!("TypeError: expected Int or Float, got {:?}", t),
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

/// Checks for a pair of numeric types (Int, Float) and returns the promoted type.
/// Promotion rules:
/// - Int, Int -> Int
/// - Float, Float -> Float
/// - Int, Float -> Float
/// - Unknown is handled optimistically.
fn expect_numeric_pair(t1: Ty, t2: Ty, span: crate::types::Span) -> SemanticResult<Ty> {
    match (t1, t2) {
        (Ty::Int, Ty::Int) => Ok(Ty::Int),
        (Ty::Float, Ty::Float) => Ok(Ty::Float),
        (Ty::Int, Ty::Float) | (Ty::Float, Ty::Int) => Ok(Ty::Float), // 혼합 연산은 Float로
        (Ty::Unknown, Ty::Int) | (Ty::Int, Ty::Unknown) => Ok(Ty::Int),
        (Ty::Unknown, Ty::Float) | (Ty::Float, Ty::Unknown) => Ok(Ty::Float),
        (Ty::Unknown, Ty::Unknown) => Ok(Ty::Unknown),
        (t1, t2) => Err(SemanticError {
            message: format!(
                "TypeError: expected numeric types, got {:?} and {:?}",
                t1, t2
            ),
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
    } else if (t1 == Ty::Int && t2 == Ty::Float) || (t1 == Ty::Float && t2 == Ty::Int) {
        // Int-Float 비교 허용
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
    let base_top = base.frames.last().expect("base.frames should always be non-empty");
    let changed_top = changed.frames.last().expect("changed.frames should always be non-empty");
    for (k, v) in changed_top.iter() {
        if base_top.get(k) != Some(v) {
            out.insert(k.clone());
        }
    }
}
fn get_var_type(env: &TypeEnv, var: &str) -> Ty {
    env.get(var).unwrap_or(Ty::Unknown)
}
