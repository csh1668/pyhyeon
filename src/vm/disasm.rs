use super::bytecode::{FunctionCode, Instruction as I, Module};
use std::fmt::{self, Write};

pub fn disassemble_module_to_string(module: &Module) -> String {
    let mut output = String::new();
    let _ = disassemble_module(module, &mut output);
    output
}

pub fn disassemble_module(module: &Module, w: &mut impl Write) -> fmt::Result {
    writeln!(w, "=== Module Disassembly ===")?;
    writeln!(w)?;

    writeln!(w, "Symbols ({}):", module.symbols.len())?;
    for (i, sym) in module.symbols.iter().enumerate() {
        writeln!(w, "  {}: \"{}\"", i, sym)?;
    }
    writeln!(w)?;

    writeln!(w, "Constants ({}):", module.consts.len())?;
    for (i, c) in module.consts.iter().enumerate() {
        writeln!(w, "  {}: {:?}", i, c)?;
    }
    writeln!(w)?;

    writeln!(w, "Classes ({}):", module.classes.len())?;
    for (i, cls) in module.classes.iter().enumerate() {
        writeln!(w, "  {}: {} - methods: {:?}", i, cls.name, cls.methods)?;
    }
    writeln!(w)?;

    writeln!(w, "Functions ({}):", module.functions.len())?;
    for (i, func) in module.functions.iter().enumerate() {
        disassemble_function(module, i, func, w)?;
        writeln!(w)?;
    }

    Ok(())
}

pub fn disassemble_function_to_string(
    module: &Module,
    func_id: usize,
    func: &FunctionCode,
) -> String {
    let mut output = String::new();
    let _ = disassemble_function(module, func_id, func, &mut output);
    output
}

pub fn disassemble_function(
    module: &Module,
    func_id: usize,
    func: &FunctionCode,
    w: &mut impl Write,
) -> fmt::Result {
    let name = &module.symbols[func.name_sym as usize];
    writeln!(
        w,
        "Function #{} - {} (arity={}, locals={})",
        func_id, name, func.arity, func.num_locals
    )?;
    writeln!(w, "  Instructions:")?;

    for (i, ins) in func.code.iter().enumerate() {
        write!(w, "    {:4}: ", i)?;
        disassemble_instruction(module, ins, w)?;
        writeln!(w)?;
    }

    Ok(())
}

fn disassemble_instruction(module: &Module, ins: &I, w: &mut impl Write) -> fmt::Result {
    let ins_name = ins.to_string();
    match ins {
        I::ConstI64(n) => write!(w, "{} {}", ins_name, n),
        I::ConstF64(f) => write!(w, "{} {}", ins_name, f),
        I::ConstStr(idx) => {
            let s = &module.string_pool[*idx as usize];
            write!(w, "{} {} (\"{}\")", ins_name, idx, s)
        }
        I::LoadConst(idx) => write!(w, "{} {}", ins_name, idx),

        I::LoadLocal(idx) => write!(w, "{} {}", ins_name, idx),
        I::StoreLocal(idx) => write!(w, "{} {}", ins_name, idx),
        I::LoadGlobal(idx) => {
            let name = &module.symbols[*idx as usize];
            write!(w, "{} {} (\"{}\")", ins_name, idx, name)
        }
        I::StoreGlobal(idx) => {
            let name = &module.symbols[*idx as usize];
            write!(w, "{} {} (\"{}\")", ins_name, idx, name)
        }

        I::Jump(offset) => write!(w, "{} {}", ins_name, offset),
        I::JumpIfFalse(offset) => write!(w, "{} {}", ins_name, offset),
        I::JumpIfTrue(offset) => write!(w, "{} {}", ins_name, offset),

        I::Call(fid, argc) => {
            let fname = &module.symbols[module.functions[*fid as usize].name_sym as usize];
            write!(
                w,
                "{} {} (func #{} \"{}\", argc={})",
                ins_name, fid, fid, fname, argc
            )
        }
        I::CallBuiltin(bid, argc) => write!(w, "{} {} (argc={})", ins_name, bid, argc),
        I::CallValue(argc) => write!(w, "{} (argc={})", ins_name, argc),
        I::CallMethod(method_sym, argc) => {
            let method_name = &module.symbols[*method_sym as usize];
            write!(
                w,
                "{} {} (\"{}\", argc={})",
                ins_name, method_sym, method_name, argc
            )
        }

        I::LoadAttr(attr_sym) => {
            let attr_name = &module.symbols[*attr_sym as usize];
            write!(w, "{} {} (\"{}\")", ins_name, attr_sym, attr_name)
        }
        I::StoreAttr(attr_sym) => {
            let attr_name = &module.symbols[*attr_sym as usize];
            write!(w, "{} {} (\"{}\")", ins_name, attr_sym, attr_name)
        }

        I::BuildList(count) => write!(w, "{} (count={})", ins_name, count),
        I::BuildDict(count) => write!(w, "{} (count={})", ins_name, count),
        
        // No arg
        _ => write!(w, "{}", ins_name),
    }
}
