use crate::vm::bytecode::{ClassDef, FunctionCode, Instruction, Module, Value};

/// Serialize a compiled Module into a Rust source function `fn make_module() -> Module`.
pub fn codegen_module(module: &Module) -> String {
    let mut out = String::new();
    out.push_str("fn make_module() -> Module {\n");

    // consts
    out.push_str("    let consts: Vec<Value> = vec![\n");
    for c in &module.consts {
        out.push_str("        ");
        out.push_str(&codegen_value(c));
        out.push_str(",\n");
    }
    out.push_str("    ];\n");

    // string_pool
    out.push_str("    let string_pool: Vec<String> = vec![\n");
    for s in &module.string_pool {
        out.push_str(&format!("        {:?}.to_string(),\n", s));
    }
    out.push_str("    ];\n");

    // globals
    out.push_str(&format!(
        "    let globals: Vec<Option<Value>> = vec![None; {}];\n",
        module.globals.len()
    ));

    // symbols
    out.push_str("    let symbols: Vec<String> = vec![\n");
    for s in &module.symbols {
        out.push_str(&format!("        {:?}.to_string(),\n", s));
    }
    out.push_str("    ];\n");

    // functions
    out.push_str("    let functions: Vec<FunctionCode> = vec![\n");
    for func in &module.functions {
        out.push_str(&codegen_function(func));
    }
    out.push_str("    ];\n");

    // classes
    out.push_str("    let classes: Vec<ClassDef> = vec![\n");
    for class in &module.classes {
        out.push_str(&codegen_class(class));
    }
    out.push_str("    ];\n");

    // types - use init_builtin_types() then add user-defined
    out.push_str("    let types = init_builtin_types();\n");

    out.push_str("    Module { consts, string_pool, globals, symbols, functions, classes, types }\n");
    out.push_str("}\n");

    out
}

fn codegen_value(v: &Value) -> String {
    match v {
        Value::Int(i) => format!("Value::Int({})", i),
        Value::Float(f) => {
            if f.is_infinite() {
                if f.is_sign_positive() {
                    "Value::Float(f64::INFINITY)".to_string()
                } else {
                    "Value::Float(f64::NEG_INFINITY)".to_string()
                }
            } else if f.is_nan() {
                "Value::Float(f64::NAN)".to_string()
            } else {
                // Use to_bits for exact float representation
                format!("Value::Float(f64::from_bits({}))", f.to_bits())
            }
        }
        Value::Bool(b) => format!("Value::Bool({})", b),
        Value::None => "Value::None".to_string(),
        Value::Object(_) => {
            // Object values shouldn't appear in consts normally
            "Value::None /* unsupported Object const */".to_string()
        }
    }
}

fn codegen_instruction(ins: &Instruction) -> String {
    match ins {
        Instruction::ConstI64(i) => format!("I::ConstI64({})", i),
        Instruction::ConstF64(f) => format!("I::ConstF64(f64::from_bits({}))", f.to_bits()),
        Instruction::ConstStr(i) => format!("I::ConstStr({})", i),
        Instruction::True => "I::True".to_string(),
        Instruction::False => "I::False".to_string(),
        Instruction::None => "I::None".to_string(),
        Instruction::LoadConst(i) => format!("I::LoadConst({})", i),
        Instruction::Pop => "I::Pop".to_string(),
        Instruction::Dup => "I::Dup".to_string(),
        Instruction::LoadLocal(i) => format!("I::LoadLocal({})", i),
        Instruction::StoreLocal(i) => format!("I::StoreLocal({})", i),
        Instruction::LoadGlobal(i) => format!("I::LoadGlobal({})", i),
        Instruction::StoreGlobal(i) => format!("I::StoreGlobal({})", i),
        Instruction::Add => "I::Add".to_string(),
        Instruction::Sub => "I::Sub".to_string(),
        Instruction::Mul => "I::Mul".to_string(),
        Instruction::Div => "I::Div".to_string(),
        Instruction::TrueDiv => "I::TrueDiv".to_string(),
        Instruction::Mod => "I::Mod".to_string(),
        Instruction::Neg => "I::Neg".to_string(),
        Instruction::Pos => "I::Pos".to_string(),
        Instruction::Eq => "I::Eq".to_string(),
        Instruction::Ne => "I::Ne".to_string(),
        Instruction::Lt => "I::Lt".to_string(),
        Instruction::Le => "I::Le".to_string(),
        Instruction::Gt => "I::Gt".to_string(),
        Instruction::Ge => "I::Ge".to_string(),
        Instruction::Not => "I::Not".to_string(),
        Instruction::Jump(i) => format!("I::Jump({})", i),
        Instruction::JumpIfFalse(i) => format!("I::JumpIfFalse({})", i),
        Instruction::JumpIfTrue(i) => format!("I::JumpIfTrue({})", i),
        Instruction::Call(f, a) => format!("I::Call({}, {})", f, a),
        Instruction::CallBuiltin(b, a) => format!("I::CallBuiltin({}, {})", b, a),
        Instruction::Return => "I::Return".to_string(),
        Instruction::CallValue(a) => format!("I::CallValue({})", a),
        Instruction::CallMethod(s, a) => format!("I::CallMethod({}, {})", s, a),
        Instruction::LoadAttr(s) => format!("I::LoadAttr({})", s),
        Instruction::StoreAttr(s) => format!("I::StoreAttr({})", s),
        Instruction::BuildList(n) => format!("I::BuildList({})", n),
        Instruction::BuildTuple(n) => format!("I::BuildTuple({})", n),
        Instruction::BuildDict(n) => format!("I::BuildDict({})", n),
        Instruction::BuildSet(n) => format!("I::BuildSet({})", n),
        Instruction::BuildTreeSet(n) => format!("I::BuildTreeSet({})", n),
        Instruction::LoadIndex => "I::LoadIndex".to_string(),
        Instruction::StoreIndex => "I::StoreIndex".to_string(),
        Instruction::MakeClosure(f, n) => format!("I::MakeClosure({}, {})", f, n),
    }
}

fn codegen_function(func: &FunctionCode) -> String {
    let mut out = String::new();
    out.push_str("        FunctionCode {\n");
    out.push_str(&format!("            name_sym: {},\n", func.name_sym));
    out.push_str(&format!("            arity: {},\n", func.arity));
    out.push_str(&format!("            num_locals: {},\n", func.num_locals));
    out.push_str("            code: vec![\n");
    for ins in &func.code {
        out.push_str(&format!("                {},\n", codegen_instruction(ins)));
    }
    out.push_str("            ],\n");
    out.push_str("        },\n");
    out
}

fn codegen_class(class: &ClassDef) -> String {
    let mut out = String::new();
    out.push_str("        ClassDef {\n");
    out.push_str(&format!("            name: {:?}.to_string(),\n", class.name));
    out.push_str("            methods: HashMap::from([\n");
    for (name, func_id) in &class.methods {
        out.push_str(&format!(
            "                ({:?}.to_string(), {}),\n",
            name, func_id
        ));
    }
    out.push_str("            ]),\n");
    out.push_str("        },\n");
    out
}
