use super::bytecode::{ClassDef, FunctionCode, Instruction as I, Module};
use crate::parser::ast::{BinaryOp, Expr, ExprS, Literal, MethodDef, Stmt, StmtS, UnaryOp};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// Loop context for tracking break/continue jumps
#[derive(Debug, Clone)]
struct LoopContext {
    /// Positions of break jumps to be patched to loop end
    break_jumps: Vec<usize>,
    /// Positions of continue jumps to be patched to loop start
    continue_jumps: Vec<usize>,
}

impl LoopContext {
    fn new() -> Self {
        Self {
            break_jumps: Vec::new(),
            continue_jumps: Vec::new(),
        }
    }
}

pub struct Compiler {
    module: Module,
    symbols: std::collections::HashMap<String, u16>,
    loop_stack: Vec<LoopContext>,
}

impl Compiler {
    pub fn new() -> Self {
        Self {
            module: Module::default(),
            symbols: Default::default(),
            loop_stack: Vec::new(),
        }
    }

    /// 기존 컨텍스트를 포함하여 컴파일러 생성 (REPL 등 증분 컴파일용)
    pub fn with_context(
        symbols: HashMap<String, u16>,
        symbol_names: Vec<String>,
        existing_functions: Vec<FunctionCode>,
    ) -> Self {
        let mut module = Module::default();

        // 심볼 테이블 초기화
        module.symbols = symbol_names;
        module.globals = vec![None; module.symbols.len()];

        // 기존 함수들 복사 (resolve_function_id에서 찾을 수 있도록)
        module.functions = existing_functions;

        Self {
            module,
            symbols,
            loop_stack: Vec::new(),
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
            self.emit_stmt(s, &mut main, None);
        }
        // implicit None return
        main.code.push(I::Return);
        // place main at index 0
        self.module.functions[0] = main;
        self.module
    }

    fn emit_block(
        &mut self,
        block: &[StmtS],
        fun: &mut FunctionCode,
        locals: Option<&HashMap<String, u16>>,
    ) {
        for s in block {
            self.emit_stmt(s, fun, locals);
        }
    }

    fn emit_stmt(
        &mut self,
        stmt: &StmtS,
        fun: &mut FunctionCode,
        locals: Option<&HashMap<String, u16>>,
    ) {
        match &stmt.0 {
            Stmt::Break => {
                // Add placeholder jump to break_jumps in current loop context
                if let Some(loop_ctx) = self.loop_stack.last_mut() {
                    loop_ctx.break_jumps.push(fun.code.len());
                    fun.code.push(I::Jump(0)); // placeholder
                } else {
                    panic!("break outside loop (should be caught by semantic analysis)");
                }
            }
            Stmt::Continue => {
                // Add placeholder jump to continue_jumps in current loop context
                if let Some(loop_ctx) = self.loop_stack.last_mut() {
                    loop_ctx.continue_jumps.push(fun.code.len());
                    fun.code.push(I::Jump(0)); // placeholder
                } else {
                    panic!("continue outside loop (should be caught by semantic analysis)");
                }
            }
            Stmt::Pass => {
                // Pass is a no-op, emit nothing
            }
            Stmt::Assign { target, value } => {
                match &target.0 {
                    Expr::Variable(name) => {
                        self.emit_expr(value, fun, locals);
                        if let Some(locals) = locals {
                            if let Some(ix) = locals.get(name) {
                                fun.code.push(I::StoreLocal(*ix));
                            } else {
                                let gid = self.sym_id(name);
                                fun.code.push(I::StoreGlobal(gid));
                            }
                        } else {
                            let gid = self.sym_id(name);
                            fun.code.push(I::StoreGlobal(gid));
                        }
                    }
                    Expr::Attribute { object, attr } => {
                        self.emit_expr(object, fun, locals);
                        self.emit_expr(value, fun, locals);
                        let attr_sym = self.intern(attr);
                        fun.code.push(I::StoreAttr(attr_sym));
                    }
                    Expr::Index { object, index } => {
                        // obj[idx] = value
                        self.emit_expr(object, fun, locals);
                        self.emit_expr(index, fun, locals);
                        self.emit_expr(value, fun, locals);
                        fun.code.push(I::StoreIndex);
                    }
                    _ => {
                        // Semantic analysis should have caught this
                        panic!("Invalid assignment target");
                    }
                }
            }
            Stmt::Expr(e) => {
                self.emit_expr(e, fun, locals);
                fun.code.push(I::Pop);
            }
            Stmt::Return(e) => {
                self.emit_expr(e, fun, locals);
                fun.code.push(I::Return);
            }
            Stmt::If {
                condition,
                then_block,
                elif_blocks,
                else_block,
            } => {
                self.emit_expr(condition, fun, locals);
                let j_if_false = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                self.emit_block(then_block, fun, locals);
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
                    self.emit_expr(cond, fun, locals);
                    let j_elif_false = fun.code.len();
                    fun.code.push(I::JumpIfFalse(0));
                    self.emit_block(block, fun, locals);
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
                    self.emit_block(block, fun, locals);
                }
                let end = fun.code.len() as i32;
                patch_rel(&mut fun.code[j_end_acc], end - (j_end_acc as i32 + 1));
            }
            Stmt::While { condition, body } => {
                // Push new loop context
                self.loop_stack.push(LoopContext::new());

                let loop_start = fun.code.len() as i32;
                self.emit_expr(condition, fun, locals);
                let j_break = fun.code.len();
                fun.code.push(I::JumpIfFalse(0));
                self.emit_block(body, fun, locals);

                // Continue jumps go back to loop start
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32;

                // Patch condition's false jump to loop end
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));

                // Pop loop context and patch all break/continue jumps
                let loop_ctx = self.loop_stack.pop().unwrap();

                // Patch all break jumps to loop end
                for break_pos in loop_ctx.break_jumps {
                    patch_rel(&mut fun.code[break_pos], end - (break_pos as i32 + 1));
                }

                // Patch all continue jumps to loop start
                for continue_pos in loop_ctx.continue_jumps {
                    patch_rel(
                        &mut fun.code[continue_pos],
                        loop_start - (continue_pos as i32 + 1),
                    );
                }
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

                // Push new loop context
                self.loop_stack.push(LoopContext::new());

                // 1. iterable.__iter__() 호출하여 iterator 생성
                self.emit_expr(iterable, fun, locals);
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

                // 7. 루프 변수에 할당 (local 또는 global)
                if let Some(locals) = locals {
                    if let Some(ix) = locals.get(var) {
                        fun.code.push(I::StoreLocal(*ix));
                    } else {
                        let var_sym = self.sym_id(var);
                        fun.code.push(I::StoreGlobal(var_sym));
                    }
                } else {
                    let var_sym = self.sym_id(var);
                    fun.code.push(I::StoreGlobal(var_sym));
                }

                // 8. body 실행
                self.emit_block(body, fun, locals);

                // 9. 루프 시작으로 jump
                let cur = fun.code.len() as i32;
                fun.code.push(I::Jump(loop_start - (cur + 1)));
                let end = fun.code.len() as i32;

                // 10. break 지점 패치
                patch_rel(&mut fun.code[j_break], end - (j_break as i32 + 1));

                // Pop loop context and patch all break/continue jumps
                let loop_ctx = self.loop_stack.pop().unwrap();

                // Patch all break jumps to loop end
                for break_pos in loop_ctx.break_jumps {
                    patch_rel(&mut fun.code[break_pos], end - (break_pos as i32 + 1));
                }

                // Patch all continue jumps to loop start
                for continue_pos in loop_ctx.continue_jumps {
                    patch_rel(
                        &mut fun.code[continue_pos],
                        loop_start - (continue_pos as i32 + 1),
                    );
                }
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
                    self.emit_stmt(s, &mut f, Some(&local_map));
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
            self.emit_stmt(s, &mut f, Some(&local_map));
        }

        // __init__ 메서드는 자동으로 self를 반환
        if method.name == "__init__" {
            f.code.push(I::LoadLocal(0)); // self는 항상 첫 번째 로컬 변수
        }

        f.code.push(I::Return);
        self.module.functions[fid] = f;

        Ok(fid as u16)
    }

    fn emit_expr(
        &mut self,
        expr: &ExprS,
        fun: &mut FunctionCode,
        locals: Option<&HashMap<String, u16>>,
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
                if let Some(locals) = locals {
                    if let Some(ix) = locals.get(name) {
                        fun.code.push(I::LoadLocal(*ix));
                    } else {
                        let gid = self.sym_id(name);
                        fun.code.push(I::LoadGlobal(gid));
                    }
                } else {
                    let gid = self.sym_id(name);
                    fun.code.push(I::LoadGlobal(gid));
                }
            }
            Expr::Unary { op, expr } => {
                self.emit_expr(expr, fun, locals);
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
                        self.emit_expr(left, fun, locals);
                        let j_false = fun.code.len();
                        fun.code.push(I::JumpIfFalse(0));
                        self.emit_expr(right, fun, locals);
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
                        self.emit_expr(left, fun, locals);
                        let j_true = fun.code.len();
                        fun.code.push(I::JumpIfTrue(0));
                        self.emit_expr(right, fun, locals);
                        let j_end = fun.code.len();
                        fun.code.push(I::Jump(0));
                        let l_true = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_true], l_true - (j_true as i32 + 1));
                        fun.code.push(I::True);
                        let l_end = fun.code.len() as i32;
                        patch_rel(&mut fun.code[j_end], l_end - (j_end as i32 + 1));
                    }
                    _ => {
                        self.emit_expr(left, fun, locals);
                        self.emit_expr(right, fun, locals);
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
                    self.emit_expr(object, fun, locals);
                    for arg in args {
                        self.emit_expr(arg, fun, locals);
                    }
                    let method_sym = self.intern(attr);
                    fun.code.push(I::CallMethod(method_sym, args.len() as u8));
                    return;
                }

                // Builtins by name (func_name이 Variable인 경우)
                if let Expr::Variable(name) = &func_name.0 {
                    if let Some(bid) = builtin_id(name) {
                        for a in args {
                            self.emit_expr(a, fun, locals);
                        }
                        fun.code.push(I::CallBuiltin(bid, args.len() as u8));
                        return;
                    }
                    // 클래스인지 확인
                    let is_class = self.module.classes.iter().any(|c| c.name == *name);
                    if is_class {
                        // 클래스는 CallValue 사용
                        self.emit_expr(func_name, fun, locals);
                        for a in args {
                            self.emit_expr(a, fun, locals);
                        }
                        fun.code.push(I::CallValue(args.len() as u8));
                        return;
                    }
                    // user function: Check if it's a defined function (def) vs variable
                    // Only use Call instruction for actual function definitions
                    let is_def_function = self.module.functions.iter().any(|f| {
                        f.name_sym < self.module.symbols.len() as u16
                            && self.module.symbols[f.name_sym as usize] == *name
                            && !self.module.symbols[f.name_sym as usize].starts_with("<lambda#")
                    });

                    if is_def_function {
                        // Direct function call
                        let fid = self.resolve_function_id(name);
                        for a in args {
                            self.emit_expr(a, fun, locals);
                        }
                        fun.code.push(I::Call(fid as u16, args.len() as u8));
                        return;
                    }

                    // Otherwise, it might be a lambda stored in a variable - use CallValue
                    // This includes variables that hold lambdas
                    self.emit_expr(func_name, fun, locals);
                    for a in args {
                        self.emit_expr(a, fun, locals);
                    }
                    fun.code.push(I::CallValue(args.len() as u8));
                    return;
                }

                // 일반적인 callable 호출: func_name을 평가한 후 CallValue
                self.emit_expr(func_name, fun, locals);
                for arg in args {
                    self.emit_expr(arg, fun, locals);
                }
                fun.code.push(I::CallValue(args.len() as u8));
            }
            Expr::Attribute { object, attr } => {
                self.emit_expr(object, fun, locals);
                let attr_sym = self.intern(attr);
                fun.code.push(I::LoadAttr(attr_sym));
            }
            Expr::List(elements) => {
                // 각 요소를 스택에 push
                for elem in elements {
                    self.emit_expr(elem, fun, locals);
                }
                // BuildList instruction
                fun.code.push(I::BuildList(elements.len() as u16));
            }
            Expr::Dict(pairs) => {
                // 각 key-value 쌍을 스택에 push (key, value 순서)
                for (key, value) in pairs {
                    self.emit_expr(key, fun, locals);
                    self.emit_expr(value, fun, locals);
                }
                // BuildDict instruction
                fun.code.push(I::BuildDict(pairs.len() as u16));
            }
            Expr::Index { object, index } => {
                // object와 index를 스택에 push
                self.emit_expr(object, fun, locals);
                self.emit_expr(index, fun, locals);
                // LoadIndex instruction
                fun.code.push(I::LoadIndex);
            }
            Expr::Lambda { params, body } => {
                // Lambda를 익명 함수로 컴파일 (Closure 지원)
                // 1. 익명 함수 이름 생성
                let lambda_name = format!("<lambda#{}>", self.module.functions.len());
                let name_sym = self.intern(&lambda_name);

                // 2. 자유 변수 분석 (외부 스코프에서 캡처해야 할 변수)
                let free_vars = collect_free_vars(body, params, locals);

                // 3. Lambda locals 레이아웃: [params..., captures...]
                let mut lambda_locals = HashMap::new();
                // 파라미터를 먼저 배치
                for (i, p) in params.iter().enumerate() {
                    lambda_locals.insert(p.clone(), i as u16);
                }
                // 캡처 변수를 파라미터 뒤에 배치
                let capture_offset = params.len() as u16;
                for (i, var) in free_vars.iter().enumerate() {
                    lambda_locals.insert(var.clone(), capture_offset + i as u16);
                }
                let num_locals = (params.len() + free_vars.len()) as u16;

                // 4. 함수 슬롯 예약
                let fid = self.module.functions.len();
                self.module.functions.push(FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![I::Return],
                });

                // 5. Lambda body 컴파일 (단일 표현식)
                let mut lambda_fun = FunctionCode {
                    name_sym,
                    arity: params.len() as u8,
                    num_locals,
                    code: vec![],
                };
                self.emit_expr(body, &mut lambda_fun, Some(&lambda_locals));
                lambda_fun.code.push(I::Return);

                // 6. 컴파일된 함수 저장
                self.module.functions[fid] = lambda_fun;

                // 7. 캡처 변수를 스택에 push (부모 함수의 locals에서)
                for var in &free_vars {
                    if let Some(&local_idx) = locals.and_then(|l| l.get(var)) {
                        fun.code.push(I::LoadLocal(local_idx));
                    } else {
                        // 이론적으로 여기 도달하면 안 됨 (자유 변수는 부모 locals에 있어야 함)
                        panic!("Free variable {} not found in parent locals", var);
                    }
                }

                // 8. MakeClosure instruction (캡처 개수 지정)
                fun.code
                    .push(I::MakeClosure(fid as u16, free_vars.len() as u8));
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

        // module.functions
        for (i, f) in self.module.functions.iter().enumerate() {
            if f.name_sym < self.module.symbols.len() as u16 {
                let sym = &self.module.symbols[f.name_sym as usize];
                if sym == name {
                    return i;
                }
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
                Stmt::Break | Stmt::Continue | Stmt::Pass => {}
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

/// Lambda body에서 참조되는 모든 변수를 수집 (재귀적)
fn collect_referenced_vars(expr: &ExprS, vars: &mut HashSet<String>) {
    match &expr.0 {
        Expr::Variable(name) => {
            vars.insert(name.clone());
        }
        Expr::Binary { left, right, .. } => {
            collect_referenced_vars(left, vars);
            collect_referenced_vars(right, vars);
        }
        Expr::Unary { expr: inner, .. } => {
            collect_referenced_vars(inner, vars);
        }
        Expr::Call { func_name, args } => {
            collect_referenced_vars(func_name, vars);
            for arg in args {
                collect_referenced_vars(arg, vars);
            }
        }
        Expr::Attribute { object, .. } => {
            collect_referenced_vars(object, vars);
        }
        Expr::List(elements) => {
            for elem in elements {
                collect_referenced_vars(elem, vars);
            }
        }
        Expr::Dict(pairs) => {
            for (key, value) in pairs {
                collect_referenced_vars(key, vars);
                collect_referenced_vars(value, vars);
            }
        }
        Expr::Index { object, index } => {
            collect_referenced_vars(object, vars);
            collect_referenced_vars(index, vars);
        }
        Expr::Lambda { params, body } => {
            // 중첩 lambda의 body도 재귀적으로 탐색
            // (중첩 lambda가 참조하는 변수를 현재 lambda도 캡처해야 할 수 있음)
            // 단, 중첩 lambda의 파라미터는 제외
            collect_referenced_vars(body, vars);
            // 중첩 lambda의 파라미터는 자유 변수가 아니므로 제거
            for param in params {
                vars.remove(param);
            }
        }
        Expr::Literal(_) => {
            // 리터럴은 변수 참조 없음
        }
    }
}

/// Lambda의 자유 변수(free variables) 분석
///
/// 자유 변수 = 참조되지만 파라미터도 아니고 로컬 변수도 아닌 변수
/// 단, 글로벌 변수는 제외 (LoadGlobal로 직접 접근 가능)
fn collect_free_vars(
    body: &ExprS,
    params: &[String],
    parent_locals: Option<&HashMap<String, u16>>,
) -> Vec<String> {
    let mut referenced_vars = HashSet::new();
    collect_referenced_vars(body, &mut referenced_vars);

    // 파라미터는 자유 변수가 아님
    for param in params {
        referenced_vars.remove(param);
    }

    // 부모 함수의 locals에 있는 변수만 캡처 대상
    // (글로벌 변수는 LoadGlobal로 접근하므로 캡처 불필요)
    let mut free_vars: Vec<String> = if let Some(locals) = parent_locals {
        referenced_vars
            .into_iter()
            .filter(|var| locals.contains_key(var))
            .collect()
    } else {
        // 부모 locals가 없으면 (모듈 레벨) 자유 변수 없음
        vec![]
    };

    // 정렬하여 일관성 보장
    free_vars.sort();
    free_vars
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
    // Use centralized builtin registry
    crate::builtins::lookup(name).map(|b| b.builtin_id)
}
