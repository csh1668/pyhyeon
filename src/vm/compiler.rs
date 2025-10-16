#![allow(dead_code)]
#![allow(unused_variables)]

use super::bytecode::{FunctionCode, Instruction as I, Module};
use crate::parser::ast::{BinaryOp, Expr, ExprS, Literal, Stmt, StmtS, UnaryOp};
use std::collections::{HashMap, HashSet};

pub struct Compiler {
    module: Module,
    symbols: std::collections::HashMap<String, u16>,
}

impl Compiler {
    pub fn new() -> Self { Self { module: Module::default(), symbols: Default::default() } }

    pub fn compile(mut self, program: &[StmtS]) -> Module {
        // Reserve function 0 for __main__ entry
        let main_sym = self.intern("__main__");
        self.module.functions.push(FunctionCode { name_sym: main_sym, arity: 0, num_locals: 0, code: vec![] });
        let mut main = FunctionCode { name_sym: main_sym, arity: 0, num_locals: 0, code: vec![] };
        for s in program { self.emit_stmt(s, &mut main); }
        // implicit None return
        main.code.push(I::Return);
        // place main at index 0
        self.module.functions[0] = main;
        self.module
    }

    fn emit_block(&mut self, block: &[StmtS], fun: &mut FunctionCode) { for s in block { self.emit_stmt(s, fun); } }

    fn emit_stmt(&mut self, stmt: &StmtS, fun: &mut FunctionCode) {
        match &stmt.0 {
            Stmt::Assign { name, value } => {
                self.emit_expr(value, fun);
                let gid = self.sym_id(name);
                fun.code.push(I::StoreGlobal(gid));
            }
            Stmt::Expr(e) => { self.emit_expr(e, fun); }
            Stmt::Return(e) => { self.emit_expr(e, fun); fun.code.push(I::Return); }
            Stmt::If { condition, then_block, elif_blocks, else_block } => {
                self.emit_expr(condition, fun);
                let j_if_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                self.emit_block(then_block, fun);
                let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                // patch first jump to else/elif start
                let else_start = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_if_false], else_start - (j_if_false as i32 + 1));
                // elifs
                let mut j_end_acc = j_end; // mutable chain
                for (cond, block) in elif_blocks {
                    self.emit_expr(cond, fun);
                    let j_elif_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                    self.emit_block(block, fun);
                    let j_after_elif = fun.code.len(); fun.code.push(I::Jump(0));
                    let after_elif_start = fun.code.len() as i32;
                    patch_rel(&mut fun.code[j_elif_false], after_elif_start - (j_elif_false as i32 + 1));
                    // chain end jumps
                    patch_chain(&mut fun.code[j_end_acc], (j_after_elif as i32) - (j_end_acc as i32 + 1));
                    j_end_acc = j_after_elif; // continue chain
                }
                // else
                if let Some(block) = else_block { self.emit_block(block, fun); }
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_end_acc], end - (j_end_acc as i32 + 1));
            }
            Stmt::While { condition, body } => {
                let loop_start = fun.code.len() as i32;
                self.emit_expr(condition, fun);
                let j_break = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                self.emit_block(body, fun);
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));
            }
            Stmt::Def { name, params, body } => {
                // compile function body with locals mapping (params + assigned names)
                let name_sym = self.intern(name);
                let local_map = collect_locals(params, body);
                let num_locals = local_map.len() as u16;
                // reserve slot for recursion resolution
                let fid = self.module.functions.len();
                self.module.functions.push(FunctionCode { name_sym, arity: params.len() as u8, num_locals, code: vec![I::Return] });
                let mut f = FunctionCode { name_sym, arity: params.len() as u8, num_locals, code: vec![] };
                for s in body { self.emit_stmt_with_locals(s, &mut f, &local_map); }
                f.code.push(I::Return);
                self.module.functions[fid] = f;
            }
        }
    }

    fn emit_expr(&mut self, expr: &ExprS, fun: &mut FunctionCode) {
        match &expr.0 {
            Expr::Literal(Literal::Int(i)) => fun.code.push(I::ConstI64(*i)),
            Expr::Literal(Literal::Bool(b)) => fun.code.push(if *b { I::True } else { I::False }),
            Expr::Literal(Literal::None) => fun.code.push(I::None),
            Expr::Variable(name) => {
                let gid = self.sym_id(name);
                fun.code.push(I::LoadGlobal(gid));
            }
            Expr::Unary { op, expr } => {
                self.emit_expr(expr, fun);
                match op {
                    UnaryOp::Not => fun.code.push(I::Not),
                    UnaryOp::Negate => fun.code.push(I::Neg),
                    UnaryOp::Pos => fun.code.push(I::Pos),
                }
            }
            Expr::Binary { op, left, right } => {
                use BinaryOp as B;
                match op {
                    B::And => {
                        // left consumed by JumpIfFalse; on false branch, re-push False
                        self.emit_expr(left, fun);
                        let j_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr(right, fun);
                        let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                        let l_false = fun.code.len() as i32; patch_rel(&mut fun.code[j_false], l_false - (j_false as i32 + 1));
                        fun.code.push(I::False);
                        let l_end = fun.code.len() as i32; patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    B::Or => {
                        // left consumed by JumpIfTrue; on true branch, re-push True
                        self.emit_expr(left, fun);
                        let j_true = fun.code.len(); fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr(right, fun);
                        let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                        let l_true = fun.code.len() as i32; patch_rel(&mut fun.code[j_true], l_true - (j_true as i32 + 1));
                        fun.code.push(I::True);
                        let l_end = fun.code.len() as i32; patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    _ => {
                        self.emit_expr(left, fun);
                        self.emit_expr(right, fun);
                        match op {
                            B::Add => fun.code.push(I::Add),
                            B::Subtract => fun.code.push(I::Sub),
                            B::Multiply => fun.code.push(I::Mul),
                            B::FloorDivide => fun.code.push(I::Div),
                            B::Modulo => fun.code.push(I::Mod),
                            B::Equal => fun.code.push(I::Eq),
                            B::NotEqual => fun.code.push(I::Ne),
                            B::Less => fun.code.push(I::Lt),
                            B::LessEqual => fun.code.push(I::Le),
                            B::Greater => fun.code.push(I::Gt),
                            B::GreaterEqual => fun.code.push(I::Ge),
                            B::And | B::Or => unreachable!(),
                        }
                    }
                }
            }
            Expr::Call { func_name, args } => {
                // Builtins by name
                if let Some(bid) = builtin_id(func_name) {
                    for a in args { self.emit_expr(a, fun); }
                    fun.code.push(I::CallBuiltin(bid, args.len() as u8));
                    return;
                }
                // user function: resolve to existing function id by name
                let fid = self.resolve_function_id(func_name);
                for a in args { self.emit_expr(a, fun); }
                fun.code.push(I::Call(fid as u16, args.len() as u8));
            }
        }
    }

    fn intern(&mut self, s: &str) -> u16 {
        if let Some(&id) = self.symbols.get(s) { return id; }
        let id = self.module.symbols.len() as u16;
        self.module.symbols.push(s.to_string());
        self.symbols.insert(s.to_string(), id);
        // also grow globals vector for new symbol
        self.module.globals.push(None);
        id
    }

    fn sym_id(&mut self, s: &str) -> u16 { self.intern(s) }

    fn resolve_function_id(&mut self, name: &str) -> usize {
        // linear scan; in v0.1 functions are compiled before use in same module body order
        if name == "__main__" { return 0; }
        for (i, f) in self.module.functions.iter().enumerate() {
            let sym = self.module.symbols[f.name_sym as usize].as_str();
            if sym == name { return i; }
        }
        // if not found, create empty stub
        let id = self.module.functions.len();
        let name_sym = self.intern(name);
        self.module.functions.push(FunctionCode { name_sym, arity: 0, num_locals: 0, code: vec![I::Return] });
        id
    }
}

impl Compiler {
    fn emit_stmt_with_locals(&mut self, stmt: &StmtS, fun: &mut FunctionCode, locals: &HashMap<String, u16>) {
        match &stmt.0 {
            Stmt::Assign { name, value } => {
                self.emit_expr_with_locals(value, fun, locals);
                if let Some(ix) = locals.get(name) { fun.code.push(I::StoreLocal(*ix)); }
                else { let gid = self.sym_id(name); fun.code.push(I::StoreGlobal(gid)); }
            }
            Stmt::Expr(e) => { self.emit_expr_with_locals(e, fun, locals); }
            Stmt::Return(e) => { self.emit_expr_with_locals(e, fun, locals); fun.code.push(I::Return); }
            Stmt::If { condition, then_block, elif_blocks, else_block } => {
                self.emit_expr_with_locals(condition, fun, locals);
                let j_if_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                for s in then_block { self.emit_stmt_with_locals(s, fun, locals); }
                let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                let else_start = fun.code.len() as i32; patch_rel(&mut fun.code[j_if_false], else_start - (j_if_false as i32 + 1));
                let mut j_end_acc = j_end;
                for (cond, block) in elif_blocks {
                    self.emit_expr_with_locals(cond, fun, locals);
                    let j_elif_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                    for s in block { self.emit_stmt_with_locals(s, fun, locals); }
                    let j_after_elif = fun.code.len(); fun.code.push(I::Jump(0));
                    let after_elif_start = fun.code.len() as i32; patch_rel(&mut fun.code[j_elif_false], after_elif_start - (j_elif_false as i32 + 1));
                    patch_chain(&mut fun.code[j_end_acc], (j_after_elif as i32) - (j_end_acc as i32 + 1));
                    j_end_acc = j_after_elif;
                }
                if let Some(block) = else_block { for s in block { self.emit_stmt_with_locals(s, fun, locals); } }
                let end = fun.code.len() as i32; patch_rel(&mut fun.code[j_end_acc], end - (j_end_acc as i32 + 1));
            }
            Stmt::While { condition, body } => {
                let loop_start = fun.code.len() as i32;
                self.emit_expr_with_locals(condition, fun, locals);
                let j_break = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                for s in body { self.emit_stmt_with_locals(s, fun, locals); }
                let cur = fun.code.len() as i32; fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32; patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));
            }
            Stmt::Def { name, params, body } => {
                // nested function
                let name_sym = self.intern(name);
                let local_map = collect_locals(params, body);
                let num_locals = local_map.len() as u16;
                let fid = self.module.functions.len();
                self.module.functions.push(FunctionCode { name_sym, arity: params.len() as u8, num_locals, code: vec![I::Return] });
                let mut f = FunctionCode { name_sym, arity: params.len() as u8, num_locals, code: vec![] };
                for s in body { self.emit_stmt_with_locals(s, &mut f, &local_map); }
                f.code.push(I::Return);
                self.module.functions[fid] = f;
            }
        }
    }

    fn emit_expr_with_locals(&mut self, expr: &ExprS, fun: &mut FunctionCode, locals: &HashMap<String, u16>) {
        match &expr.0 {
            Expr::Literal(Literal::Int(i)) => fun.code.push(I::ConstI64(*i)),
            Expr::Literal(Literal::Bool(b)) => fun.code.push(if *b { I::True } else { I::False }),
            Expr::Literal(Literal::None) => fun.code.push(I::None),
            Expr::Variable(name) => {
                if let Some(ix) = locals.get(name) { fun.code.push(I::LoadLocal(*ix)); }
                else { let gid = self.sym_id(name); fun.code.push(I::LoadGlobal(gid)); }
            }
            Expr::Unary { op, expr } => {
                self.emit_expr_with_locals(expr, fun, locals);
                match op { UnaryOp::Not => fun.code.push(I::Not), UnaryOp::Negate => fun.code.push(I::Neg), UnaryOp::Pos => fun.code.push(I::Pos) }
            }
            Expr::Binary { op, left, right } => {
                use BinaryOp as B;
                match op {
                    B::And => {
                        self.emit_expr_with_locals(left, fun, locals);
                        let j_false = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr_with_locals(right, fun, locals);
                        let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                        let l_false = fun.code.len() as i32; patch_rel(&mut fun.code[j_false], l_false - (j_false as i32 + 1));
                        fun.code.push(I::False);
                        let l_end = fun.code.len() as i32; patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    B::Or => {
                        self.emit_expr_with_locals(left, fun, locals);
                        let j_true = fun.code.len(); fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr_with_locals(right, fun, locals);
                        let j_end = fun.code.len(); fun.code.push(I::Jump(0));
                        let l_true = fun.code.len() as i32; patch_rel(&mut fun.code[j_true], l_true - (j_true as i32 + 1));
                        fun.code.push(I::True);
                        let l_end = fun.code.len() as i32; patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    _ => {
                        self.emit_expr_with_locals(left, fun, locals);
                        self.emit_expr_with_locals(right, fun, locals);
                        match op { B::Add => fun.code.push(I::Add), B::Subtract => fun.code.push(I::Sub), B::Multiply => fun.code.push(I::Mul), B::FloorDivide => fun.code.push(I::Div), B::Modulo => fun.code.push(I::Mod), B::Equal => fun.code.push(I::Eq), B::NotEqual => fun.code.push(I::Ne), B::Less => fun.code.push(I::Lt), B::LessEqual => fun.code.push(I::Le), B::Greater => fun.code.push(I::Gt), B::GreaterEqual => fun.code.push(I::Ge), B::And | B::Or => unreachable!(), }
                    }
                }
            }
            Expr::Call { func_name, args } => {
                if let Some(bid) = builtin_id(func_name) { for a in args { self.emit_expr_with_locals(a, fun, locals); } fun.code.push(I::CallBuiltin(bid, args.len() as u8)); return; }
                let fid = self.resolve_function_id(func_name);
                for a in args { self.emit_expr_with_locals(a, fun, locals); }
                fun.code.push(I::Call(fid as u16, args.len() as u8));
            }
        }
    }
}

fn collect_locals(params: &Vec<String>, body: &Vec<StmtS>) -> HashMap<String, u16> {
    let mut map: HashMap<String, u16> = HashMap::new();
    for (i, p) in params.iter().enumerate() { map.insert(p.clone(), i as u16); }
    let mut seen: HashSet<String> = params.iter().cloned().collect();
    fn walk(body: &Vec<StmtS>, seen: &mut HashSet<String>) {
        for s in body {
            match &s.0 {
                Stmt::Assign { name, .. } => { seen.insert(name.clone()); }
                Stmt::Def { name, .. } => { seen.insert(name.clone()); }
                Stmt::If { then_block, elif_blocks, else_block, .. } => {
                    walk(then_block, seen);
                    for (_, b) in elif_blocks { walk(b, seen); }
                    if let Some(b) = else_block { walk(b, seen); }
                }
                Stmt::While { body, .. } => { walk(body, seen); }
                Stmt::Return(_) | Stmt::Expr(_) => {}
            }
        }
    }
    walk(body, &mut seen);
    for name in seen { if !map.contains_key(&name) { let idx = map.len() as u16; map.insert(name, idx); } }
    map
}

fn patch_rel(ins: &mut I, rel: i32) {
    match ins { I::JumpIfFalse(r) | I::JumpIfTrue(r) | I::Jump(r) => *r = rel, _ => unreachable!("not a jump") }
}

fn patch_chain(ins: &mut I, rel: i32) { patch_rel(ins, rel); }

fn builtin_id(name: &str) -> Option<u8> {
    match name {
        "print" => Some(0),
        "input" => Some(1),
        "int" => Some(2),
        "bool" => Some(3),
        _ => None,
    }
}


