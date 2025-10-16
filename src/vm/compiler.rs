#![allow(dead_code)]
#![allow(unused_variables)]

use super::bytecode::{FunctionCode, Instruction as I, Module};
use crate::parser::ast::{BinaryOp, Expr, ExprS, Literal, Stmt, StmtS, UnaryOp};

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
                // compile function body as a separate FunctionCode
                let name_sym = self.intern(name);
                let mut f = FunctionCode { name_sym, arity: params.len() as u8, num_locals: params.len() as u16, code: vec![] };
                // body
                for s in body { self.emit_stmt(s, &mut f); }
                f.code.push(I::Return);
                let fid = self.module.functions.len() as u16;
                self.module.functions.push(f);
                // in main, store function id to global by name (placeholder: we don't have first-class functions; VM Call uses func_id directly)
                // For v0.1, calls are by name, compiler will emit Call with func_id resolved here
                // So nothing to emit at def site for globals
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
                        // short-circuit: if left is false, push false and skip right
                        self.emit_expr(left, fun);
                        let j = fun.code.len(); fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr(right, fun);
                        let end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j], end - (j as i32 + 1));
                    }
                    B::Or => {
                        // short-circuit: if left is true, push true and skip right
                        self.emit_expr(left, fun);
                        let j = fun.code.len(); fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr(right, fun);
                        let end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j], end - (j as i32 + 1));
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


