use pyhyeon as lib;
use std::env;

fn main() {
    // engine selection: default interpreter; override with --engine=vm or --engine=interp
    let mut engine = String::from("interp");
    for arg in env::args().skip(1) {
        if arg == "--engine=vm" { engine = "vm".into(); }
        if arg == "--engine=interp" { engine = "interp".into(); }
    }
    // subcommands per plan.md: run/repl/compile/exec (minimal: run/compile/exec)
    // Usage examples:
    //   pyh run program.pyh [--engine=vm|interp]
    //   pyh compile program.pyh -o out.pyhb
    //   pyh exec out.pyhb
    let mut args = env::args().skip(1).collect::<Vec<String>>();
    let mut subcmd = "run".to_string();
    let mut input_path = "./test.pyh".to_string();
    let mut out_path: Option<String> = None;
    if !args.is_empty() {
        let first = &args[0];
        if ["run","compile","exec","repl"].contains(&first.as_str()) { subcmd = first.clone(); args.remove(0); }
    }
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            s if s.starts_with("--engine=") => { /* already handled */ }
            "-o" => { if i+1 < args.len() { out_path = Some(args[i+1].clone()); i+=1; } }
            p => { input_path = p.to_string(); }
        }
        i+=1;
    }
    let path = input_path.as_str();
    let src = std::fs::read_to_string(path).expect("Failed to read source file");
    // add newline if not present at the end of file
    let src = if src.ends_with('\n') { src } else { format!("{}\n", src) };

    match subcmd.as_str() {
        "run" => {
            if let Ok(program) = lib::parse_source(path, &src) {
                if lib::analyze(&program, path, &src) {
                    match engine.as_str() {
                        "vm" => { let module = lib::compile_to_module(&program); lib::exec_vm_module(module); }
                        _ => { lib::run_interpreter(&program, path, &src); }
                    }
                }
            }
        }
        "compile" => {
            if let Ok(program) = lib::parse_source(path, &src) {
                if lib::analyze(&program, path, &src) {
                    let module = lib::compile_to_module(&program);
                    let out = out_path.as_deref().unwrap_or("out.pyhb");
                    lib::save_module(&module, out).expect("failed to save module");
                    println!("wrote {}", out);
                }
            }
        }
        "exec" => {
            let module = lib::load_module(path).expect("failed to load module");
            lib::exec_vm_module(module);
        }
        "repl" => {
            eprintln!("REPL is not implemented yet.");
        }
        _ => {}
    }
}
