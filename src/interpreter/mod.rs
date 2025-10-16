use std::collections::HashMap;

use crate::parser::ast::{BinaryOp, Expr, ExprS, Literal, Stmt, StmtS, UnaryOp};
use crate::builtins;
use crate::types::Span;

#[derive(Debug, Clone)]
pub enum Value {
    Int(i64),
    Bool(bool),
    None,
    Function(Function),
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<StmtS>,
    pub env: Env, // lexical environment snapshot
}

#[derive(Debug, Clone, Default)]
pub struct Env {
    // first = global, last = current
    frames: Vec<HashMap<String, Value>>,
}

impl Env {
    pub fn new() -> Self { Self { frames: vec![HashMap::new()] } }
    pub fn push(&mut self) { self.frames.push(HashMap::new()); }
    pub fn pop(&mut self) { self.frames.pop(); }
    pub fn define(&mut self, name: String, val: Value, span: Span) -> Result<(), RuntimeError> {
        if let Some(cur) = self.frames.last_mut() { cur.insert(name, val); return Ok(()); }
        Err(RuntimeError { message: format!("failed to define '{}': no frame", name), span })
    }
    pub fn assign(&mut self, name: &str, val: Value, span: Span) -> Result<(), RuntimeError> {
        for f in self.frames.iter_mut().rev() {
            if f.contains_key(name) { f.insert(name.to_string(), val); return Ok(()); }
        }
        // define in current if not found (python-like)
        self.define(name.to_string(), val, span)
    }
    pub fn get(&self, name: &str) -> Option<Value> {
        for f in self.frames.iter().rev() {
            if let Some(v) = f.get(name) { return Some(v.clone()); }
        }
        None
    }
}

#[derive(Debug)]
pub struct RuntimeError { pub message: String, pub span: Span }

type RtResult<T> = Result<T, RuntimeError>;

pub struct Interpreter {
    pub env: Env,
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

impl Interpreter {
    pub fn new() -> Self {
        let mut env = Env::new();
        for b in builtins::all() {
            let params: Vec<String> = b.params.iter().map(|&s| (*s).to_string()).collect();
            env.define(b.name.to_string(), Value::Function(Function { name: b.name.to_string(), params, body: vec![], env: Env::new() }), 0..0).unwrap();
        }
        Self { env }
    }

    pub fn run(&mut self, program: &[StmtS]) -> RtResult<()> {
        for stmt in program {
            self.exec_stmt(stmt)?;
        }
        Ok(())
    }

    fn exec_block(&mut self, block: &[StmtS]) -> RtResult<Option<Value>> {
        for s in block {
            if let Some(v) = self.exec_stmt(s)? { return Ok(Some(v)); }
        }
        Ok(None)
    }

    fn exec_stmt(&mut self, stmt: &StmtS) -> RtResult<Option<Value>> {
        match &stmt.0 {
            Stmt::Assign { name, value } => {
                let v = self.eval_expr(value)?;
                self.env.assign(name, v, stmt.1.clone())?;
                Ok(None)
            }
            Stmt::Expr(expr) => { let _ = self.eval_expr(expr)?; Ok(None) }
            Stmt::Return(expr) => { let v = self.eval_expr(expr)?; Ok(Some(v)) }
            Stmt::If { condition, then_block, elif_blocks, else_block } => {
                let cond = self.eval_expr(condition)?;
                if to_bool(&cond, condition.1.clone())? {
                    return self.exec_block(then_block);
                }
                for (c, b) in elif_blocks {
                    let cv = self.eval_expr(c)?;
                    if to_bool(&cv, c.1.clone())? { return self.exec_block(b); }
                }
                if let Some(b) = else_block { return self.exec_block(b); }
                Ok(None)
            }
            Stmt::While { condition, body } => {
                loop {
                    let c = self.eval_expr(condition)?;
                    if !to_bool(&c, condition.1.clone())? { break; }
                    if let Some(v) = self.exec_block(body)? { return Ok(Some(v)); }
                }
                Ok(None)
            }
            Stmt::Def { name, params, body } => {
                // Capture current env snapshot for lexical scoping (no mutation of outer via closure in v0.1)
                let func = Function { name: name.clone(), params: params.clone(), body: body.clone(), env: self.env.clone() };
                self.env.define(name.clone(), Value::Function(func), stmt.1.clone())?;
                Ok(None)
            }
        }
    }

    fn eval_expr(&mut self, expr: &ExprS) -> RtResult<Value> {
        match &expr.0 {
            Expr::Literal(Literal::Int(i)) => Ok(Value::Int(*i)),
            Expr::Literal(Literal::Bool(b)) => Ok(Value::Bool(*b)),
            Expr::Literal(Literal::None) => Ok(Value::None),
            Expr::Variable(name) => self.env.get(name).ok_or_else(|| RuntimeError { message: format!("NameError: name '{}' is not defined", name), span: expr.1.clone() }),
            Expr::Unary { op, expr } => {
                let v = self.eval_expr(expr)?;
                match op {
                    UnaryOp::Not => Ok(Value::Bool(!to_bool(&v, expr.1.clone())?)),
                    UnaryOp::Negate => Ok(Value::Int(-to_int(&v, expr.1.clone())?)),
                    UnaryOp::Pos => Ok(Value::Int(to_int(&v, expr.1.clone())?)),
                }
            }
            Expr::Binary { op, left, right } => {
                match op {
                    BinaryOp::And => {
                        let lv = self.eval_expr(left)?; // short-circuit
                        if !to_bool(&lv, left.1.clone())? { return Ok(Value::Bool(false)); }
                        let rv = self.eval_expr(right)?;
                        return Ok(Value::Bool(to_bool(&rv, right.1.clone())?));
                    }
                    BinaryOp::Or => {
                        let lv = self.eval_expr(left)?; // short-circuit
                        if to_bool(&lv, left.1.clone())? { return Ok(Value::Bool(true)); }
                        let rv = self.eval_expr(right)?;
                        return Ok(Value::Bool(to_bool(&rv, right.1.clone())?));
                    }
                    _ => {}
                }
                let lv = self.eval_expr(left)?;
                let rv = self.eval_expr(right)?;
                use BinaryOp as B;
                match op {
                    B::Add => Ok(Value::Int(to_int(&lv, left.1.clone())? + to_int(&rv, right.1.clone())?)),
                    B::Subtract => Ok(Value::Int(to_int(&lv, left.1.clone())? - to_int(&rv, right.1.clone())?)),
                    B::Multiply => Ok(Value::Int(to_int(&lv, left.1.clone())? * to_int(&rv, right.1.clone())?)),
                    B::FloorDivide => {
                        let a = to_int(&lv, left.1.clone())?; let b = to_int(&rv, right.1.clone())?;
                        if b == 0 { return Err(RuntimeError { message: "ZeroDivisionError: integer division by zero".into(), span: expr.1.clone() }); }
                        Ok(Value::Int(a / b))
                    }
                    B::Modulo => {
                        let a = to_int(&lv, left.1.clone())?; let b = to_int(&rv, right.1.clone())?;
                        if b == 0 { return Err(RuntimeError { message: "ZeroDivisionError: integer modulo by zero".into(), span: expr.1.clone() }); }
                        Ok(Value::Int(a % b))
                    }
                    B::Equal => Ok(Value::Bool(equals(&lv, &rv))),
                    B::NotEqual => Ok(Value::Bool(!equals(&lv, &rv))),
                    B::Less => Ok(Value::Bool(to_int(&lv, left.1.clone())? < to_int(&rv, right.1.clone())?)),
                    B::LessEqual => Ok(Value::Bool(to_int(&lv, left.1.clone())? <= to_int(&rv, right.1.clone())?)),
                    B::Greater => Ok(Value::Bool(to_int(&lv, left.1.clone())? > to_int(&rv, right.1.clone())?)),
                    B::GreaterEqual => Ok(Value::Bool(to_int(&lv, left.1.clone())? >= to_int(&rv, right.1.clone())?)),
                    B::And | B::Or => unreachable!(),
                }
            }
            Expr::Call { func_name, args } => self.call(func_name, args, expr.1.clone()),
        }
    }

    fn call(&mut self, func_name: &str, args: &Vec<ExprS>, call_span: Span) -> RtResult<Value> {
        // builtins
        match func_name {
            "print" => {
                if args.len() != 1 { return Err(RuntimeError { message: format!("ArityError: print() takes 1 positional argument but {} given", args.len()), span: call_span.clone() }); }
                let v = self.eval_expr(&args[0])?;
                println!("{}", display_value(&v));
                return Ok(Value::None);
            }
            "input" => {
                if !args.is_empty() { return Err(RuntimeError { message: format!("ArityError: input() takes 0 positional arguments but {} given", args.len()), span: call_span.clone() }); }
                // Simple line-based input (reads one line)
                use std::io::{self, Read};
                let mut buf = String::new();
                // On Windows console, read_line would be more appropriate, but stdin buffered read suffices here
                io::stdin().read_to_string(&mut buf).map_err(|e| RuntimeError { message: format!("IOError: {}", e), span: call_span.clone() })?;
                let line = buf.lines().next().unwrap_or("");
                let parsed = line.trim().parse::<i64>().map_err(|_| RuntimeError { message: "ValueError: input() expects an integer line".into(), span: call_span.clone() })?;
                return Ok(Value::Int(parsed));
            }
            "int" => {
                if args.len() != 1 { return Err(RuntimeError { message: format!("ArityError: int() takes 1 positional argument but {} given", args.len()), span: call_span.clone() }); }
                let v = self.eval_expr(&args[0])?;
                return Ok(Value::Int(to_int(&v, args[0].1.clone())?));
            }
            "bool" => {
                if args.len() != 1 { return Err(RuntimeError { message: format!("ArityError: bool() takes 1 positional argument but {} given", args.len()), span: call_span.clone() }); }
                let v = self.eval_expr(&args[0])?;
                return Ok(Value::Bool(to_bool(&v, args[0].1.clone())?));
            }
            _ => {}
        }

        // user-defined
        let fv = self.env.get(func_name).ok_or_else(|| RuntimeError { message: format!("NameError: function '{}' is not defined", func_name), span: call_span.clone() })?;
        let func = match fv { Value::Function(f) => f, _ => return Err(RuntimeError { message: format!("TypeError: '{}' is not callable", func_name), span: call_span.clone() }) };
        if func.params.len() != args.len() {
            return Err(RuntimeError { message: format!("ArityError: function '{}' takes {} positional arguments but {} were given", func.name, func.params.len(), args.len()), span: call_span.clone() });
        }

        // evaluate arguments
        let mut arg_vals = Vec::with_capacity(args.len());
        for a in args { arg_vals.push(self.eval_expr(a)?); }

        // new frame with captured env baseline
        let saved_env = self.env.clone();
        self.env = func.env.clone();
        self.env.push();
        // bind function name to itself in the current frame to enable recursion
        self.env.define(func.name.clone(), Value::Function(func.clone()), call_span.clone())?;
        for (p, v) in func.params.iter().zip(arg_vals.into_iter()) { self.env.define(p.clone(), v, call_span.clone())?; }
        let ret = self.exec_block(&func.body)?;
        // restore caller env
        self.env = saved_env;
        Ok(ret.unwrap_or(Value::None))
    }
}

fn to_int(v: &Value, span: Span) -> RtResult<i64> {
    match v { Value::Int(i) => Ok(*i), Value::Bool(b) => Ok(if *b { 1 } else { 0 }), _ => Err(RuntimeError { message: format!("TypeError: expected Int-compatible, got {:?}", v), span }) }
}
fn to_bool(v: &Value, span: Span) -> RtResult<bool> {
    match v { Value::Bool(b) => Ok(*b), Value::Int(i) => Ok(*i != 0), Value::None => Ok(false), Value::Function(_) => Err(RuntimeError { message: "TypeError: cannot convert function to bool".into(), span }) }
}
fn equals(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::None, Value::None) => true,
        _ => false,
    }
}
fn display_value(v: &Value) -> String {
    match v {
        Value::Int(i) => i.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::None => "None".into(),
        Value::Function(f) => format!("<function {}>", f.name),
    }
}


