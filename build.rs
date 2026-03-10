//! Build script: generates standalone_runtime.rs from VM source files.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("standalone_runtime.rs");

    let mut output = String::new();

    // Header
    output.push_str(
        r#"#![allow(unused, dead_code, unreachable_patterns, non_snake_case, irrefutable_let_patterns)]
use std::cell::RefCell;
use std::collections::{HashMap, HashSet, BTreeSet, VecDeque};
use std::rc::Rc;
use std::io::{self, BufRead, Write};
use std::ops::Bound;
"#,
    );

    // TypeFlags replacement
    output.push_str(
        r#"
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TypeFlags(pub u32);
impl TypeFlags {
    pub const IMMUTABLE: TypeFlags = TypeFlags(1 << 0);
    pub const CALLABLE: TypeFlags = TypeFlags(1 << 1);
    pub const ITERABLE: TypeFlags = TypeFlags(1 << 2);
    pub fn contains(self, other: TypeFlags) -> bool { self.0 & other.0 == other.0 }
    pub fn empty() -> TypeFlags { TypeFlags(0) }
}
impl std::ops::BitOr for TypeFlags {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self { TypeFlags(self.0 | rhs.0) }
}
impl std::ops::BitOrAssign for TypeFlags {
    fn bitor_assign(&mut self, rhs: Self) { self.0 |= rhs.0; }
}
"#,
    );

    // Top-level files (no name collisions between these)
    let top_level = [
        "src/builtins.rs",
        "src/vm/type_def.rs",
        "src/vm/bytecode.rs",
        "src/vm/value.rs",
        "src/runtime_io.rs",
        "src/vm/utils.rs",
    ];

    for path in &top_level {
        emit_file(&mut output, path, None);
    }

    // Machine files: method_dispatch and instruction are impl blocks on Vm, not standalone fns
    // They can go top-level since they only add methods to Vm
    emit_file(&mut output, "src/vm/machine/mod.rs", None);
    emit_file(&mut output, "src/vm/machine/instruction.rs", None);
    emit_file(&mut output, "src/vm/machine/method_dispatch.rs", None);

    // Builtin files need module wrapping due to name collisions (call, register_type, etc.)
    let builtin_files: &[(&str, &str)] = &[
        ("print", "src/vm/builtins/print.rs"),
        ("input", "src/vm/builtins/input.rs"),
        ("int", "src/vm/builtins/int.rs"),
        ("bool_mod", "src/vm/builtins/bool.rs"),
        ("str_mod", "src/vm/builtins/str.rs"),
        ("str_methods", "src/vm/builtins/str_methods.rs"),
        ("float", "src/vm/builtins/float.rs"),
        ("len", "src/vm/builtins/len.rs"),
        ("range", "src/vm/builtins/range.rs"),
        ("list", "src/vm/builtins/list.rs"),
        ("list_methods", "src/vm/builtins/list_methods.rs"),
        ("dict", "src/vm/builtins/dict.rs"),
        ("dict_methods", "src/vm/builtins/dict_methods.rs"),
        ("assert_mod", "src/vm/builtins/assert.rs"),
        ("none_mod", "src/vm/builtins/none.rs"),
        ("map", "src/vm/builtins/map.rs"),
        ("filter", "src/vm/builtins/filter.rs"),
        ("set", "src/vm/builtins/set.rs"),
        ("set_methods", "src/vm/builtins/set_methods.rs"),
        ("treeset", "src/vm/builtins/treeset.rs"),
        ("treeset_methods", "src/vm/builtins/treeset_methods.rs"),
    ];

    for (mod_name, path) in builtin_files {
        emit_file(&mut output, path, Some(mod_name));
    }

    // Re-export functions used across builtin modules
    output.push_str("use bool_mod::to_bool;\n");

    // Type alias used by method_dispatch.rs
    output.push_str("type NM = NativeMethod;\n");

    // Instruction alias used by instruction.rs
    output.push_str("type I = Instruction;\n");

    // Builtin dispatcher that delegates to the sub-modules
    // This replaces src/vm/builtins/mod.rs
    output.push_str(
        r#"
// ===== Builtin Dispatcher =====
fn call_builtin<IO: RuntimeIo>(id: u8, args: Vec<Value>, io: &mut IO) -> VmResult<Value> {
    match id {
        BUILTIN_PRINT_ID => print::call(args, io),
        BUILTIN_INPUT_ID => input::call(args, io),
        BUILTIN_INT_ID => int::call(args),
        BUILTIN_BOOL_ID => bool_mod::call(args),
        BUILTIN_STR_ID => str_methods::call(args),
        BUILTIN_LEN_ID => len::call(args),
        BUILTIN_RANGE_ID => range::create_range(args),
        BUILTIN_FLOAT_ID => float::call(args),
        BUILTIN_MAP_ID => map::create_map(args),
        BUILTIN_FILTER_ID => filter::create_filter(args),
        BUILTIN_SET_ID => set::call(args),
        BUILTIN_TREESET_ID => treeset::call(args),
        BUILTIN_ASSERT_ID => assert_mod::call(args),
        _ => Err(err(
            VmErrorKind::TypeError("builtin"),
            format!("unknown builtin id {}", id),
        )),
    }
}
"#,
    );

    fs::write(&out_path, output).expect("Failed to write standalone_runtime.rs");
}

fn emit_file(output: &mut String, file_path: &str, wrap_module: Option<&str>) {
    println!("cargo:rerun-if-changed={}", file_path);
    let content = fs::read_to_string(file_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", file_path, e));

    output.push_str(&format!("\n// ===== {} =====\n", file_path));

    let transformed = transform_source(&content);

    if let Some(mod_name) = wrap_module {
        output.push_str(&format!("pub mod {} {{\n", mod_name));
        output.push_str("    use super::*;\n");
        for line in transformed.lines() {
            if line.is_empty() {
                output.push('\n');
            } else {
                output.push_str("    ");
                output.push_str(line);
                output.push('\n');
            }
        }
        output.push_str("}\n");
    } else {
        output.push_str(&transformed);
        output.push('\n');
    }
}

fn transform_source(source: &str) -> String {
    #![allow(clippy::manual_strip)]
    let mut result = String::new();
    let lines: Vec<&str> = source.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Skip crate/super/serde/strum/bitflags use statements (multi-line)
        if trimmed.starts_with("use crate::")
            || trimmed.starts_with("use super::")
            || trimmed.starts_with("pub use crate::")
            || trimmed.starts_with("pub use super::")
            || trimmed.starts_with("use serde")
            || trimmed.starts_with("use strum")
            || trimmed.starts_with("use bitflags")
        {
            while i < lines.len() && !lines[i].contains(';') {
                i += 1;
            }
            i += 1;
            continue;
        }

        // Skip mod declarations
        if (trimmed.starts_with("pub mod ") || trimmed.starts_with("mod "))
            && trimmed.ends_with(';')
        {
            i += 1;
            continue;
        }

        // Skip #![allow(...)]
        if trimmed.starts_with("#![allow(") {
            i += 1;
            continue;
        }

        // Convert //! to //
        if trimmed.starts_with("//!") {
            result.push_str(&format!("//{}", &trimmed[3..]));
            result.push('\n');
            i += 1;
            continue;
        }

        // Skip std imports (already in header)
        if trimmed.starts_with("use std::") {
            while i < lines.len() && !lines[i].contains(';') {
                i += 1;
            }
            i += 1;
            continue;
        }

        // Remove #[cfg(test)] mod tests { ... } or #[cfg(test)] mod tests;
        if trimmed == "#[cfg(test)]" {
            if i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if next.starts_with("mod tests") {
                    if next.ends_with(';') {
                        // `mod tests;` - just skip the 2 lines
                        i += 2;
                    } else {
                        // `mod tests { ... }` - skip the brace block
                        i = skip_brace_block(&lines, i + 1);
                    }
                    continue;
                }
            }
        }

        // Remove #[cfg(target_arch = "wasm32")] blocks
        if trimmed.starts_with("#[cfg(target_arch = \"wasm32\")]") {
            if i + 1 < lines.len() {
                let next = lines[i + 1].trim();
                if next.starts_with("mod ") || next.starts_with("pub ") || next == "{" {
                    i = skip_brace_block(&lines, i + 1);
                    continue;
                }
            }
            i += 1;
            continue;
        }

        // Remove bitflags! { ... }
        if trimmed.starts_with("bitflags::bitflags!") || trimmed == "bitflags! {" {
            i = skip_brace_block(&lines, i);
            continue;
        }

        // Transform derive macros
        if trimmed.starts_with("#[derive(") {
            if let Some(nl) = transform_derive(line) {
                result.push_str(&nl);
                result.push('\n');
            }
            i += 1;
            continue;
        }

        // Remove #[serde(...)]
        if trimmed.starts_with("#[serde(") {
            i += 1;
            continue;
        }

        // Replace pub(super) with pub
        let mut line = if line.contains("pub(super)") {
            line.replace("pub(super)", "pub")
        } else {
            line.to_string()
        };

        // Replace crate:: and super:: path prefixes with nothing
        // These references need to become direct since everything is in one file
        line = replace_crate_paths(&line);

        result.push_str(&line);
        result.push('\n');
        i += 1;
    }

    result
}

/// Replace crate:: and super:: path prefixes to flatten for standalone mode.
/// Strips module path prefixes so that items resolve at the top level.
fn replace_crate_paths(line: &str) -> String {
    let mut result = line.to_string();

    // Strip these path prefixes (order: longest first to avoid partial matches)
    let prefixes = [
        "super::super::super::builtins::",
        "super::super::builtins::",
        "super::super::bytecode::",
        "super::super::type_def::",
        "super::super::utils::",
        "super::super::value::",
        "super::super::",
        "super::bytecode::",
        "super::type_def::",
        "super::utils::",
        "super::value::",
        "crate::vm::bytecode::",
        "crate::vm::type_def::",
        "crate::vm::utils::",
        "crate::vm::value::",
        "crate::vm::machine::",
        "crate::vm::builtins::",
        "crate::vm::",
        "crate::builtins::",
        "crate::runtime_io::",
        // instruction module reference (used as `use instruction::ExecutionFlow`)
        "instruction::",
        // Bare super:: references in top-level machine files
        "super::instruction::",
        // machine module reference (map/filter builtins use machine::Vm)
        "machine::",
    ];

    for prefix in &prefixes {
        result = result.replace(prefix, "");
    }

    // Specific replacements (not just prefix stripping)
    result = result.replace("super::ExecutionFlow", "ExecutionFlow");
    // bool module was renamed to bool_mod to avoid keyword collision
    result = result.replace("bool::", "bool_mod::");

    result
}

fn skip_brace_block(lines: &[&str], start: usize) -> usize {
    let mut depth = 0i32;
    let mut found_open = false;
    let mut i = start;
    while i < lines.len() {
        for ch in lines[i].chars() {
            if ch == '{' {
                depth += 1;
                found_open = true;
            } else if ch == '}' {
                depth -= 1;
                if found_open && depth == 0 {
                    return i + 1;
                }
            }
        }
        i += 1;
    }
    i
}

fn transform_derive(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(start) = trimmed.find("#[derive(") {
        if let Some(end) = trimmed.rfind(")]") {
            let inner = &trimmed[start + "#[derive(".len()..end];
            let kept: Vec<&str> = inner
                .split(',')
                .map(|s| s.trim())
                .filter(|s| {
                    !s.is_empty()
                        && *s != "Serialize"
                        && *s != "Deserialize"
                        && *s != "Display"
                })
                .collect();
            if kept.is_empty() {
                return None;
            }
            let indent = &line[..line.len() - line.trim_start().len()];
            return Some(format!("{}#[derive({})]", indent, kept.join(", ")));
        }
    }
    Some(line.to_string())
}
