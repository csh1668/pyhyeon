#![allow(dead_code)]
#![allow(unused_variables)]

use super::bytecode::{ClassDef, FunctionCode, Instruction as I, Module};
use crate::parser::ast::{BinaryOp, Expr, ExprS, Literal, MethodDef, Stmt, StmtS, UnaryOp};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

pub struct Compiler {
    module: Module,
    symbols: std::collections::HashMap<String, u16>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            module: Module::default(),
            symbols: Default::default(),
        }
    }

    pub fn compile(mut self, program: &[StmtS]) -> Module {
        // Reserve function 0 for __main__ entry
        let main_sym = self.intern("__main__");
        self.module.functions.push(FunctionCode {
            name_sym: main_sym,
            arity: 0,
            num_locals: 0,
            code: vec![],
        });
        let mut main = FunctionCode {
            name_sym: main_sym,
            arity: 0,
            num_locals: 0,
            code: vec![],
        };
        for s in program {
            self.emit_stmt(s, &mut main);
        }
        // implicit None return
        main.code.push(I::Return);
        // place main at index 0
        self.module.functions[0] = main;
        self.module
    }

    fn emit_block(&mut self, block: &[StmtS], fun: &mut FunctionCode) {
        for s in block {
            self.emit_stmt(s, fun);
        }
    }

    fn emit_stmt(&mut self, stmt: &StmtS, fun: &mut FunctionCode) {
        match &stmt.0 {
            Stmt::Assign { target, value } => {
                match &target.0 {
                    Expr::Variable(name) => {
                        self.emit_expr(value, fun);
                        let gid = self.sym_id(name);
                        fun.code.push(I::StoreGlobal(gid));
                    }
                    Expr::Attribute { object, attr } => {
                        self.emit_expr(object, fun);
                        self.emit_expr(value, fun);
                        let attr_sym = self.intern(attr);
                        fun.code.push(I::StoreAttr(attr_sym));
                    }
                    Expr::Index { object, index } => {
                        // obj[idx] = value
                        self.emit_expr(object, fun);
                        self.emit_expr(index, fun);
                        self.emit_expr(value, fun);
                        fun.code.push(I::StoreIndex);
                    }
                    _ => {
                        // Semantic analysis should have caught this
                        panic!("Invalid assignment target");
                    }
                }
            }
            Stmt::Expr(e) => {
                self.emit_expr(e, fun);
                fun.code.push(I::Pop);
            }
            Stmt::Return(e) => {
                self.emit_expr(e, fun);
                fun.code.push(I::Return);
            }
            Stmt::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.emit_expr(condition, fun);
                let j_if_false = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                self.emit_block(then_block, fun);
                let j_end = fun.code.len();
                fun.code.push(I::Jump(0));
                // patch first jump to else/elif start
                let else_start = fun.code.len() as i32;
                patch_rel(
                    &mut fun.code[j_if_false],
                    else_start - (j_if_false as i32 + 1),
                );
                // elifs
                let mut j_end_acc = j_end; // mutable chain
                for (cond, block) in elif_blocks {
                    self.emit_expr(cond, fun);
                    let j_elif_false = fun.code.len();
                    fun.code.push(I::JumpIfFalse(0));
                    self.emit_block(block, fun);
                    let j_after_elif = fun.code.len();
                    fun.code.push(I::Jump(0));
                    let after_elif_start = fun.code.len() as i32;
                    patch_rel(
                        &mut fun.code[j_elif_false],
                        after_elif_start - (j_elif_false as i32 + 1),
                    );
                    // chain end jumps
                    patch_chain(
                        &mut fun.code[j_end_acc],
                        (j_after_elif as i32) - (j_end_acc as i32 + 1),
                    );
                    j_end_acc = j_after_elif; // continue chain
                }
                // else
                if let Some(block) = else_block {
                    self.emit_block(block, fun);
                }
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_end_acc], end - (j_end_acc as i32 + 1));
            }
            Stmt::While { condition, body } => {
                let loop_start = fun.code.len() as i32;
                self.emit_expr(condition, fun);
                let j_break = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                self.emit_block(body, fun);
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));
            }
            Stmt::For {
                var,
                iterable,
                body,
            } => {
                // for문 desugaring:
                // for var in iterable:
                //     body
                // =>
                // __iter_temp__ = iterable.__iter__()
                // while __iter_temp__.__has_next__():
                //     var = __iter_temp__.__next__()
                //     body

                // 1. iterable.__iter__() 호출하여 iterator 생성
                self.emit_expr(iterable, fun);
                let iter_method = self.intern("__iter__");
                fun.code.push(I::CallMethod(iter_method, 0));

                // 2. iterator를 임시 global 변수에 저장
                let iter_temp_name = format!("__for_iter_{}__", fun.code.len());
                let iter_temp_sym = self.intern(&iter_temp_name);
                fun.code.push(I::StoreGlobal(iter_temp_sym));

                // 3. while 루프 시작
                let loop_start = fun.code.len() as i32;

                // 4. __iter_temp__.__has_next__() 호출
                fun.code.push(I::LoadGlobal(iter_temp_sym));
                let has_next_method = self.intern("__has_next__");
                fun.code.push(I::CallMethod(has_next_method, 0));

                // 5. has_next가 false면 루프 종료
                let j_break = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));

                // 6. __iter_temp__.__next__() 호출하여 값 가져오기
                fun.code.push(I::LoadGlobal(iter_temp_sym));
                let next_method = self.intern("__next__");
                fun.code.push(I::CallMethod(next_method, 0));

                // 7. 루프 변수에 할당
                let var_sym = self.sym_id(var);
                fun.code.push(I::StoreGlobal(var_sym));

                // 8. body 실행
                self.emit_block(body, fun);

                // 9. 루프 시작으로 jump
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));

                // 10. break 지점 패치
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
                self.module.functions.push(FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![I::Return],
                });
                let mut f = FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![],
                };
                for s in body {
                    self.emit_stmt_with_locals(s, &mut f, &local_map);
                }
                f.code.push(I::Return);
                self.module.functions[fid] = f;
            }
            Stmt::Class { name, methods, .. } => {
                // 먼저 빈 ClassDef를 module.classes에 추가
                let class_id = self.module.classes.len();
                let class_def = ClassDef {
                    name: name.clone(),
                    methods: HashMap::new(), // 나중에 업데이트
                };
                self.module.classes.push(class_def);

                // 각 메서드를 함수로 컴파일
                let mut method_map = HashMap::new();
                for method in methods {
                    let method_func_id = self.compile_method(method).unwrap();
                    method_map.insert(method.name.clone(), method_func_id);
                }

                // ClassDef 업데이트
                self.module.classes[class_id].methods = method_map.clone();

                // Phase 4: UserClass를 Object로 저장
                let name_sym = self.intern(name);
                let const_id = self.module.consts.len();
                let class_obj = super::bytecode::Value::Object(Rc::new(super::value::Object::new(
                    super::type_def::TYPE_USER_START + class_id as u16,
                    super::value::ObjectData::UserClass {
                        class_id: class_id as u16,
                        methods: method_map,
                    },
                )));
                self.module.consts.push(class_obj);
                fun.code.push(I::LoadConst(const_id as u32));
                fun.code.push(I::StoreGlobal(name_sym));
            }
        }
    }

    fn compile_method(&mut self, method: &MethodDef) -> Result<u16, String> {
        let name_sym = self.intern(&method.name);
        let local_map = collect_locals(&method.params, &method.body);
        let num_locals = local_map.len() as u16;

        let fid = self.module.functions.len();
        self.module.functions.push(FunctionCode {
            name_sym,
            arity: method.params.len() as u8,
            num_locals,
            code: vec![I::Return],
        });

        let mut f = FunctionCode {
            name_sym,
            arity: method.params.len() as u8,
            num_locals,
            code: vec![],
        };

        for s in &method.body {
            self.emit_stmt_with_locals(s, &mut f, &local_map);
        }

        // __init__ 메서드는 자동으로 self를 반환
        if method.name == "__init__" {
            f.code.push(I::LoadLocal(0)); // self는 항상 첫 번째 로컬 변수
        }

        f.code.push(I::Return);
        self.module.functions[fid] = f;

        Ok(fid as u16)
    }

    fn emit_expr(&mut self, expr: &ExprS, fun: &mut FunctionCode) {
        match &expr.0 {
            Expr::Literal(Literal::Int(i)) => fun.code.push(I::ConstI64(*i)),
            Expr::Literal(Literal::Float(f)) => fun.code.push(I::ConstF64(*f)),
            Expr::Literal(Literal::Bool(b)) => fun.code.push(if *b { I::True } else { I::False }),
            Expr::Literal(Literal::String(s)) => {
                let str_id = get_or_add_string(&mut self.module, s.clone());
                fun.code.push(I::ConstStr(str_id));
            }
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
                        let j_false = fun.code.len();
                        fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr(right, fun);
                        let j_end = fun.code.len();
                        fun.code.push(I::Jump(0));
                        let l_false = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_false], l_false - (j_false as i32 + 1));
                        fun.code.push(I::False);
                        let l_end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    B::Or => {
                        // left consumed by JumpIfTrue; on true branch, re-push True
                        self.emit_expr(left, fun);
                        let j_true = fun.code.len();
                        fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr(right, fun);
                        let j_end = fun.code.len();
                        fun.code.push(I::Jump(0));
                        let l_true = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_true], l_true - (j_true as i32 + 1));
                        fun.code.push(I::True);
                        let l_end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    _ => {
                        self.emit_expr(left, fun);
                        self.emit_expr(right, fun);
                        match op {
                            B::Add => fun.code.push(I::Add),
                            B::Subtract => fun.code.push(I::Sub),
                            B::Multiply => fun.code.push(I::Mul),
                            B::Divide => fun.code.push(I::TrueDiv),
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
                // 특별 처리: func_name이 Attribute인 경우 → CallMethod 최적화
                if let Expr::Attribute { object, attr } = &func_name.0 {
                    // 메서드 호출: obj.method(args)
                    self.emit_expr(object, fun);
                    for arg in args {
                        self.emit_expr(arg, fun);
                    }
                    let method_sym = self.intern(attr);
                    fun.code.push(I::CallMethod(method_sym, args.len() as u8));
                    return;
                }

                // Builtins by name (func_name이 Variable인 경우)
                if let Expr::Variable(name) = &func_name.0 {
                    if let Some(bid) = builtin_id(name) {
                        for a in args {
                            self.emit_expr(a, fun);
                        }
                        fun.code.push(I::CallBuiltin(bid, args.len() as u8));
                        return;
                    }
                    // 클래스인지 확인
                    let is_class = self.module.classes.iter().any(|c| c.name == *name);
                    if is_class {
                        // 클래스는 CallValue 사용
                        self.emit_expr(func_name, fun);
                        for a in args {
                            self.emit_expr(a, fun);
                        }
                        fun.code.push(I::CallValue(args.len() as u8));
                        return;
                    }
                    // user function: resolve to existing function id by name
                    let fid = self.resolve_function_id(name);
                    for a in args {
                        self.emit_expr(a, fun);
                    }
                    fun.code.push(I::Call(fid as u16, args.len() as u8));
                    return;
                }

                // 일반적인 callable 호출: func_name을 평가한 후 CallValue
                self.emit_expr(func_name, fun);
                for arg in args {
                    self.emit_expr(arg, fun);
                }
                fun.code.push(I::CallValue(args.len() as u8));
            }
            Expr::Attribute { object, attr } => {
                self.emit_expr(object, fun);
                let attr_sym = self.intern(attr);
                fun.code.push(I::LoadAttr(attr_sym));
            }
            Expr::List(elements) => {
                // 각 요소를 스택에 push
                for elem in elements {
                    self.emit_expr(elem, fun);
                }
                // BuildList instruction
                fun.code.push(I::BuildList(elements.len() as u16));
            }
            Expr::Dict(pairs) => {
                // 각 key-value 쌍을 스택에 push (key, value 순서)
                for (key, value) in pairs {
                    self.emit_expr(key, fun);
                    self.emit_expr(value, fun);
                }
                // BuildDict instruction
                fun.code.push(I::BuildDict(pairs.len() as u16));
            }
            Expr::Index { object, index } => {
                // object와 index를 스택에 push
                self.emit_expr(object, fun);
                self.emit_expr(index, fun);
                // LoadIndex instruction
                fun.code.push(I::LoadIndex);
            }
        }
    }

    fn intern(&mut self, s: &str) -> u16 {
        if let Some(&id) = self.symbols.get(s) {
            return id;
        }
        let id = self.module.symbols.len() as u16;
        self.module.symbols.push(s.to_string());
        self.symbols.insert(s.to_string(), id);
        // also grow globals vector for new symbol
        self.module.globals.push(None);
        id
    }

    fn sym_id(&mut self, s: &str) -> u16 {
        self.intern(s)
    }

    fn resolve_function_id(&mut self, name: &str) -> usize {
        // linear scan; in v0.1 functions are compiled before use in same module body order
        if name == "__main__" {
            return 0;
        }
        for (i, f) in self.module.functions.iter().enumerate() {
            let sym = self.module.symbols[f.name_sym as usize].as_str();
            if sym == name {
                return i;
            }
        }
        // if not found, create empty stub
        let id = self.module.functions.len();
        let name_sym = self.intern(name);
        self.module.functions.push(FunctionCode {
            name_sym,
            arity: 0,
            num_locals: 0,
            code: vec![I::Return],
        });
        id
    }
}

impl Compiler {
    fn emit_stmt_with_locals(
        &mut self,
        stmt: &StmtS,
        fun: &mut FunctionCode,
        locals: &HashMap<String, u16>,
    ) {
        match &stmt.0 {
            Stmt::Assign { target, value } => {
                match &target.0 {
                    Expr::Variable(name) => {
                        self.emit_expr_with_locals(value, fun, locals);
                        if let Some(ix) = locals.get(name) {
                            fun.code.push(I::StoreLocal(*ix));
                        } else {
                            let gid = self.sym_id(name);
                            fun.code.push(I::StoreGlobal(gid));
                        }
                    }
                    Expr::Attribute { object, attr } => {
                        self.emit_expr_with_locals(object, fun, locals);
                        self.emit_expr_with_locals(value, fun, locals);
                        let attr_sym = self.intern(attr);
                        fun.code.push(I::StoreAttr(attr_sym));
                    }
                    Expr::Index { object, index } => {
                        // obj[idx] = value
                        self.emit_expr_with_locals(object, fun, locals);
                        self.emit_expr_with_locals(index, fun, locals);
                        self.emit_expr_with_locals(value, fun, locals);
                        fun.code.push(I::StoreIndex);
                    }
                    _ => {
                        // Semantic analysis should have caught this
                        panic!("Invalid assignment target");
                    }
                }
            }
            Stmt::Expr(e) => {
                self.emit_expr_with_locals(e, fun, locals);
                fun.code.push(I::Pop);
            }
            Stmt::Return(e) => {
                self.emit_expr_with_locals(e, fun, locals);
                fun.code.push(I::Return);
            }
            Stmt::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.emit_expr_with_locals(condition, fun, locals);
                let j_if_false = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                for s in then_block {
                    self.emit_stmt_with_locals(s, fun, locals);
                }
                let j_end = fun.code.len();
                fun.code.push(I::Jump(0));
                let else_start = fun.code.len() as i32;
                patch_rel(
                    &mut fun.code[j_if_false],
                    else_start - (j_if_false as i32 + 1),
                );
                let mut j_end_acc = j_end;
                for (cond, block) in elif_blocks {
                    self.emit_expr_with_locals(cond, fun, locals);
                    let j_elif_false = fun.code.len();
                    fun.code.push(I::JumpIfFalse(0));
                    for s in block {
                        self.emit_stmt_with_locals(s, fun, locals);
                    }
                    let j_after_elif = fun.code.len();
                    fun.code.push(I::Jump(0));
                    let after_elif_start = fun.code.len() as i32;
                    patch_rel(
                        &mut fun.code[j_elif_false],
                        after_elif_start - (j_elif_false as i32 + 1),
                    );
                    patch_chain(
                        &mut fun.code[j_end_acc],
                        (j_after_elif as i32) - (j_end_acc as i32 + 1),
                    );
                    j_end_acc = j_after_elif;
                }
                if let Some(block) = else_block {
                    for s in block {
                        self.emit_stmt_with_locals(s, fun, locals);
                    }
                }
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_end_acc], end - (j_end_acc as i32 + 1));
            }
            Stmt::While { condition, body } => {
                let loop_start = fun.code.len() as i32;
                self.emit_expr_with_locals(condition, fun, locals);
                let j_break = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                for s in body {
                    self.emit_stmt_with_locals(s, fun, locals);
                }
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));
            }
            Stmt::For {
                var,
                iterable,
                body,
            } => {
                // for문 desugaring (locals 버전)

                // 1. iterable.__iter__() 호출하여 iterator 생성
                self.emit_expr_with_locals(iterable, fun, locals);
                let iter_method = self.intern("__iter__");
                fun.code.push(I::CallMethod(iter_method, 0));

                // 2. iterator를 임시 변수에 저장 (local 또는 global)
                let iter_temp_name = format!("__for_iter_{}__", fun.code.len());
                let iter_temp_sym = self.intern(&iter_temp_name);

                // 임시 변수는 항상 global로 저장 (locals map에 없음)
                fun.code.push(I::StoreGlobal(iter_temp_sym));

                // 3. while 루프 시작
                let loop_start = fun.code.len() as i32;

                // 4. __iter_temp__.__has_next__() 호출
                fun.code.push(I::LoadGlobal(iter_temp_sym));
                let has_next_method = self.intern("__has_next__");
                fun.code.push(I::CallMethod(has_next_method, 0));

                // 5. has_next가 false면 루프 종료
                let j_break = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));

                // 6. __iter_temp__.__next__() 호출하여 값 가져오기
                fun.code.push(I::LoadGlobal(iter_temp_sym));
                let next_method = self.intern("__next__");
                fun.code.push(I::CallMethod(next_method, 0));

                // 7. 루프 변수에 할당 (local 또는 global)
                if let Some(ix) = locals.get(var) {
                    fun.code.push(I::StoreLocal(*ix));
                } else {
                    let var_sym = self.sym_id(var);
                    fun.code.push(I::StoreGlobal(var_sym));
                }

                // 8. body 실행
                for s in body {
                    self.emit_stmt_with_locals(s, fun, locals);
                }

                // 9. 루프 시작으로 jump
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));

                // 10. break 지점 패치
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));
            }
            Stmt::Def { name, params, body } => {
                // nested function
                let name_sym = self.intern(name);
                let local_map = collect_locals(params, body);
                let num_locals = local_map.len() as u16;
                let fid = self.module.functions.len();
                self.module.functions.push(FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![I::Return],
                });
                let mut f = FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![],
                };
                for s in body {
                    self.emit_stmt_with_locals(s, &mut f, &local_map);
                }
                f.code.push(I::Return);
                self.module.functions[fid] = f;
            }
            Stmt::Class { name, methods, .. } => {
                // 각 메서드를 함수로 컴파일
                let mut method_map = HashMap::new();
                for method in methods {
                    let method_func_id = self.compile_method(method).unwrap();
                    method_map.insert(method.name.clone(), method_func_id);
                }

                // ClassDef 생성
                let class_def = ClassDef {
                    name: name.clone(),
                    methods: method_map,
                };

                // Module.classes에 추가
                let class_id = self.module.classes.len();
                self.module.classes.push(class_def.clone());

                // Phase 4: UserClass를 Object로 저장
                let const_id = self.module.consts.len();
                let class_obj = super::bytecode::Value::Object(Rc::new(super::value::Object::new(
                    super::type_def::TYPE_USER_START + class_id as u16,
                    super::value::ObjectData::UserClass {
                        class_id: class_id as u16,
                        methods: class_def.methods.clone(),
                    },
                )));
                self.module.consts.push(class_obj);
                fun.code.push(I::LoadConst(const_id as u32));

                if let Some(ix) = locals.get(name) {
                    fun.code.push(I::StoreLocal(*ix));
                } else {
                    let name_sym = self.intern(name);
                    fun.code.push(I::StoreGlobal(name_sym));
                }
            }
        }
    }

    fn emit_expr_with_locals(
        &mut self,
        expr: &ExprS,
        fun: &mut FunctionCode,
        locals: &HashMap<String, u16>,
    ) {
        match &expr.0 {
            Expr::Literal(Literal::Int(i)) => fun.code.push(I::ConstI64(*i)),
            Expr::Literal(Literal::Float(f)) => fun.code.push(I::ConstF64(*f)),
            Expr::Literal(Literal::Bool(b)) => fun.code.push(if *b { I::True } else { I::False }),
            Expr::Literal(Literal::String(s)) => {
                let str_id = get_or_add_string(&mut self.module, s.clone());
                fun.code.push(I::ConstStr(str_id));
            }
            Expr::Literal(Literal::None) => fun.code.push(I::None),
            Expr::Variable(name) => {
                if let Some(ix) = locals.get(name) {
                    fun.code.push(I::LoadLocal(*ix));
                } else {
                    let gid = self.sym_id(name);
                    fun.code.push(I::LoadGlobal(gid));
                }
            }
            Expr::Unary { op, expr } => {
                self.emit_expr_with_locals(expr, fun, locals);
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
                        self.emit_expr_with_locals(left, fun, locals);
                        let j_false = fun.code.len();
                        fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr_with_locals(right, fun, locals);
                        let j_end = fun.code.len();
                        fun.code.push(I::Jump(0));
                        let l_false = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_false], l_false - (j_false as i32 + 1));
                        fun.code.push(I::False);
                        let l_end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    B::Or => {
                        self.emit_expr_with_locals(left, fun, locals);
                        let j_true = fun.code.len();
                        fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr_with_locals(right, fun, locals);
                        let j_end = fun.code.len();
                        fun.code.push(I::Jump(0));
                        let l_true = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_true], l_true - (j_true as i32 + 1));
                        fun.code.push(I::True);
                        let l_end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    _ => {
                        self.emit_expr_with_locals(left, fun, locals);
                        self.emit_expr_with_locals(right, fun, locals);
                        match op {
                            B::Add => fun.code.push(I::Add),
                            B::Subtract => fun.code.push(I::Sub),
                            B::Multiply => fun.code.push(I::Mul),
                            B::Divide => fun.code.push(I::TrueDiv),
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
                // 특별 처리: func_name이 Attribute인 경우 → CallMethod 최적화
                if let Expr::Attribute { object, attr } = &func_name.0 {
                    // 메서드 호출: obj.method(args)
                    self.emit_expr_with_locals(object, fun, locals);
                    for arg in args {
                        self.emit_expr_with_locals(arg, fun, locals);
                    }
                    let method_sym = self.intern(attr);
                    fun.code.push(I::CallMethod(method_sym, args.len() as u8));
                    return;
                }

                // Builtins by name (func_name이 Variable인 경우)
                if let Expr::Variable(name) = &func_name.0 {
                    if let Some(bid) = builtin_id(name) {
                        for a in args {
                            self.emit_expr_with_locals(a, fun, locals);
                        }
                        fun.code.push(I::CallBuiltin(bid, args.len() as u8));
                        return;
                    }
                    // 클래스인지 확인
                    let is_class = self.module.classes.iter().any(|c| c.name == *name);
                    if is_class {
                        // 클래스는 CallValue 사용
                        self.emit_expr_with_locals(func_name, fun, locals);
                        for a in args {
                            self.emit_expr_with_locals(a, fun, locals);
                        }
                        fun.code.push(I::CallValue(args.len() as u8));
                        return;
                    }
                    // user function: resolve to existing function id by name
                    let fid = self.resolve_function_id(name);
                    for a in args {
                        self.emit_expr_with_locals(a, fun, locals);
                    }
                    fun.code.push(I::Call(fid as u16, args.len() as u8));
                    return;
                }

                // 일반적인 callable 호출: func_name을 평가한 후 CallValue
                self.emit_expr_with_locals(func_name, fun, locals);
                for arg in args {
                    self.emit_expr_with_locals(arg, fun, locals);
                }
                fun.code.push(I::CallValue(args.len() as u8));
            }
            Expr::Attribute { object, attr } => {
                self.emit_expr_with_locals(object, fun, locals);
                let attr_sym = self.intern(attr);
                fun.code.push(I::LoadAttr(attr_sym));
            }
            Expr::List(elements) => {
                // 각 요소를 스택에 push
                for elem in elements {
                    self.emit_expr_with_locals(elem, fun, locals);
                }
                // BuildList instruction
                fun.code.push(I::BuildList(elements.len() as u16));
            }
            Expr::Dict(pairs) => {
                // 각 key-value 쌍을 스택에 push (key, value 순서)
                for (key, value) in pairs {
                    self.emit_expr_with_locals(key, fun, locals);
                    self.emit_expr_with_locals(value, fun, locals);
                }
                // BuildDict instruction
                fun.code.push(I::BuildDict(pairs.len() as u16));
            }
            Expr::Index { object, index } => {
                // object와 index를 스택에 push
                self.emit_expr_with_locals(object, fun, locals);
                self.emit_expr_with_locals(index, fun, locals);
                // LoadIndex instruction
                fun.code.push(I::LoadIndex);
            }
        }
    }
}

fn collect_locals(params: &[String], body: &[StmtS]) -> HashMap<String, u16> {
    let mut map: HashMap<String, u16> = HashMap::new();
    for (i, p) in params.iter().enumerate() {
        map.insert(p.clone(), i as u16);
    }
    let mut seen: HashSet<String> = params.iter().cloned().collect();
    fn walk(body: &[StmtS], seen: &mut HashSet<String>) {
        for s in body {
            match &s.0 {
                Stmt::Assign { target, .. } => {
                    // locals는 Variable만 추적
                    if let Expr::Variable(name) = &target.0 {
                        seen.insert(name.clone());
                    }
                }
                Stmt::Def { name, .. } => {
                    seen.insert(name.clone());
                }
                Stmt::Class { name, .. } => {
                    seen.insert(name.clone());
                }
                Stmt::If {
                    then_block,
                    elif_blocks,
                    else_block,
                    ..
                } => {
                    walk(then_block, seen);
                    for (_, b) in elif_blocks {
                        walk(b, seen);
                    }
                    if let Some(b) = else_block {
                        walk(b, seen);
                    }
                }
                Stmt::While { body, .. } => {
                    walk(body, seen);
                }
                Stmt::For { var, body, .. } => {
                    // for문의 루프 변수도 local로 수집
                    seen.insert(var.clone());
                    walk(body, seen);
                }
                Stmt::Return(_) | Stmt::Expr(_) => {}
            }
        }
    }
    walk(body, &mut seen);
    for name in seen {
        if !map.contains_key(&name) {
            let idx = map.len() as u16;
            map.insert(name, idx);
        }
    }
    map
}

fn patch_rel(ins: &mut I, rel: i32) {
    match ins {
        I::JumpIfFalse(r) | I::JumpIfTrue(r) | I::Jump(r) => *r = rel,
        _ => unreachable!("not a jump"),
    }
}

fn patch_chain(ins: &mut I, rel: i32) {
    patch_rel(ins, rel);
}

fn get_or_add_string(module: &mut Module, s: String) -> u32 {
    if let Some(idx) = module.string_pool.iter().position(|x| x == &s) {
        idx as u32
    } else {
        let id = module.string_pool.len() as u32;
        module.string_pool.push(s);
        id
    }
}

fn builtin_id(name: &str) -> Option<u8> {
    match name {
        "print" => Some(0),
        "input" => Some(1),
        "int" => Some(2),
        "bool" => Some(3),
        "str" => Some(4),
        "len" => Some(5),
        "range" => Some(6),
        "float" => Some(7),
        _ => None,
    }
}
