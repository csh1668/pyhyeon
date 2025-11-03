use super::bytecode::{Instruction as I, Module, FunctionCode};
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

pub fn disassemble_function_to_string(module: &Module, func_id: usize, func: &FunctionCode) -> String {
    let mut output = String::new();
    let _ = disassemble_function(module, func_id, func, &mut output);
    output
}

pub fn disassemble_function(module: &Module, func_id: usize, func: &FunctionCode, w: &mut impl Write) -> fmt::Result {
    let name = &module.symbols[func.name_sym as usize];
    writeln!(w, "Function #{} - {} (arity={}, locals={})", func_id, name, func.arity, func.num_locals)?;
    writeln!(w, "  Instructions:")?;
    
    for (i, ins) in func.code.iter().enumerate() {
        write!(w, "    {:4}: ", i)?;
        disassemble_instruction(module, ins, w)?;
        writeln!(w)?;
    }
    
    Ok(())
}

fn disassemble_instruction(module: &Module, ins: &I, w: &mut impl Write) -> fmt::Result {
    match ins {
        I::ConstI64(n) => write!(w, "ConstI64 {}", n),
        I::ConstStr(idx) => {
            let s = &module.string_pool[*idx as usize];
            write!(w, "ConstStr {} (\"{}\")", idx, s)
        }
        I::LoadConst(idx) => write!(w, "LoadConst {}", idx),
        I::True => write!(w, "True"),
        I::False => write!(w, "False"),
        I::None => write!(w, "None"),
        
        I::LoadLocal(idx) => write!(w, "LoadLocal {}", idx),
        I::StoreLocal(idx) => write!(w, "StoreLocal {}", idx),
        I::LoadGlobal(idx) => {
            let name = &module.symbols[*idx as usize];
            write!(w, "LoadGlobal {} (\"{}\")", idx, name)
        }
        I::StoreGlobal(idx) => {
            let name = &module.symbols[*idx as usize];
            write!(w, "StoreGlobal {} (\"{}\")", idx, name)
        }
        
        I::Add => write!(w, "Add"),
        I::Sub => write!(w, "Sub"),
        I::Mul => write!(w, "Mul"),
        I::Div => write!(w, "Div"),
        I::Mod => write!(w, "Mod"),
        I::Neg => write!(w, "Neg"),
        I::Pos => write!(w, "Pos"),
        
        I::Eq => write!(w, "Eq"),
        I::Ne => write!(w, "Ne"),
        I::Lt => write!(w, "Lt"),
        I::Le => write!(w, "Le"),
        I::Gt => write!(w, "Gt"),
        I::Ge => write!(w, "Ge"),
        I::Not => write!(w, "Not"),
        
        I::Jump(offset) => write!(w, "Jump {}", offset),
        I::JumpIfFalse(offset) => write!(w, "JumpIfFalse {}", offset),
        I::JumpIfTrue(offset) => write!(w, "JumpIfTrue {}", offset),
        
        I::Call(fid, argc) => {
            let fname = &module.symbols[module.functions[*fid as usize].name_sym as usize];
            write!(w, "Call {} (func #{} \"{}\", argc={})", fid, fid, fname, argc)
        }
        I::CallBuiltin(bid, argc) => write!(w, "CallBuiltin {} (argc={})", bid, argc),
        I::CallValue(argc) => write!(w, "CallValue (argc={})", argc),
        I::CallMethod(method_sym, argc) => {
            let method_name = &module.symbols[*method_sym as usize];
            write!(w, "CallMethod {} (\"{}\", argc={})", method_sym, method_name, argc)
        }
        I::Return => write!(w, "Return"),
        
        I::LoadAttr(attr_sym) => {
            let attr_name = &module.symbols[*attr_sym as usize];
            write!(w, "LoadAttr {} (\"{}\")", attr_sym, attr_name)
        }
        I::StoreAttr(attr_sym) => {
            let attr_name = &module.symbols[*attr_sym as usize];
            write!(w, "StoreAttr {} (\"{}\")", attr_sym, attr_name)
        }
        
        I::BuildList(count) => write!(w, "BuildList (count={})", count),
        I::BuildDict(count) => write!(w, "BuildDict (count={})", count),
        I::LoadIndex => write!(w, "LoadIndex"),
        I::StoreIndex => write!(w, "StoreIndex"),
    }
}

