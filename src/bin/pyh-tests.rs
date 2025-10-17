use std::fs;
use std::io;
use std::path::Path;

use pyhyeon as lib;
use std::time::Instant;

fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let filter = args.first().map(|s| s.as_str());
    let dir = Path::new("tests/programs");
    if !dir.exists() {
        eprintln!("tests/programs not found.");
        return Ok(());
    }
    let mut entries = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "pyh").unwrap_or(false))
        .collect::<Vec<_>>();
    entries.sort();

    for path in entries {
        if let Some(f) = filter
            && !path.to_string_lossy().contains(f)
        {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();
        println!("==== [{}] ====", path_str);
        let mut src = fs::read_to_string(&path_str)?;
        if !src.ends_with('\n') {
            src.push('\n');
        }

        let program = match lib::parse_source(&src) {
            Ok(p) => p,
            Err(diagnostics) => {
                for diag in diagnostics {
                    eprint!("{}", diag.format(&path_str, &src, "Parsing failed", 3));
                }
                continue;
            }
        };

        if let Err(diag) = lib::analyze(&program) {
            eprint!(
                "{}",
                diag.format(&path_str, &src, "Semantic Analyzing Failed", 4)
            );
            continue;
        }

        println!("-- interpreter --");
        let t0 = Instant::now();
        if let Err(diag) = lib::run_interpreter(&program) {
            eprint!("{}", diag.format(&path_str, &src, "Runtime Error", 5));
        }
        let interp_ms = t0.elapsed().as_millis();
        println!("[interp] {} ms", interp_ms);

        println!("-- vm --");
        let t1 = Instant::now();
        let module = lib::compile_to_module(&program);
        let compile_ms = t1.elapsed().as_millis();
        let t2 = Instant::now();
        lib::exec_vm_module(module);
        let exec_ms = t2.elapsed().as_millis();
        println!(
            "[vm] compile={} ms, exec={} ms, total={} ms",
            compile_ms,
            exec_ms,
            compile_ms + exec_ms
        );
        println!();
    }

    Ok(())
}
