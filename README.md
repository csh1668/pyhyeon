# Pyhyeon

<div align="center">

[![Rust](https://img.shields.io/badge/rust-1.84%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Playground](https://img.shields.io/badge/try-playground-brightgreen.svg)](https://csh1668.github.io/pyhyeon/)

**A subset language of Python**

</div>

---

## 🎯 Overview

Pyhyeon is a toy programming language implementing Python's core syntax. It provides a complete language implementation pipeline: Lexer, Parser, Semantic Analyzer, Interpreter, Bytecode Compiler, and VM.

### ✨ Features

- ✅ **Complete Compiler Pipeline**: Lexing → Parsing → Semantic Analysis → Execution
- ✅ **Dual Execution Engines**: Tree-walking Interpreter & Stack-based VM
- ✅ **Python-style Syntax**: Indentation-based blocks, functions, control flow
- ✅ **Type Safety**: Static semantic analysis and type checking
- ✅ **Friendly Error Messages**: Ariadne-based error reporting
- ✅ **[Web Playground](https://csh1668.github.io/pyhyeon/)**: WASM-based browser execution environment
- ✅ **Bytecode Compilation**: Compile to `.pyhb` files

### 📝 Example

```python
def fib(n):
  if n < 2:
    return n
  return fib(n-1) + fib(n-2)

print(fib(10))  # 55
```

```python
# Variables, control flow, operators
x = 10
if x > 5:
  print(x * 2)
else:
  print(x)

# Loops
i = 0
while i < 5:
  print(i)
  i = i + 1
```

## 🚀 Quick Start

### Requirements

- Rust 1.84+ (2024 edition)

### Installation

```bash
git clone https://github.com/csh1668/pyhyeon.git
cd pyhyeon
cargo build --release
```

### Usage

```bash
# Run with interpreter
cargo run --release -- run examples/fib.pyh

# Run with VM (faster)
cargo run --release -- run examples/fib.pyh --engine=vm

# Compile to bytecode
cargo run --release -- compile examples/fib.pyh -o fib.pyhb

# Execute compiled bytecode
cargo run --release -- exec fib.pyhb
```

## 📚 Language Features

### Data Types
- `int` (64-bit integer)
- `bool` (`True`, `False`)
- `None`

### Operators
- **Arithmetic**: `+`, `-`, `*`, `//` (floor division), `%`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
- **Logical**: `and`, `or`, `not` (with short-circuit evaluation)
- **Unary**: `+`, `-`, `not`

### Control Structures
- `if` / `elif` / `else`
- `while` loops
- Function definitions (`def`) with recursion support

### Built-in Functions
- `print(x)` - Output
- `input()` - Integer input
- `int(x)` - Integer conversion
- `bool(x)` - Boolean conversion

**More features will be added soon!**

## 🏗️ Architecture

```
Source Code (.pyh)
    ↓
Lexer (logos) → Token Stream
    ↓
Parser (chumsky) → AST
    ↓
Semantic Analyzer → Validated AST
    ↓        ↓
Interpreter  Compiler → Bytecode (.pyhb)
    ↓        ↓
  Execute    VM → Execute
```

## 🧪 Testing

```bash
# Unit tests
cargo test

# Integration tests (performance comparison)
cargo run --release --bin pyh-tests

# Test specific program
cargo run --release --bin pyh-tests fib
```

## 🌐 Web Playground

Try it online: **[https://csh1668.github.io/pyhyeon/](https://csh1668.github.io/pyhyeon/)**

To run locally:

```bash
cd web
cargo install wasm-pack # Install once for first run
pnpm install
pnpm wasm
pnpm dev
```

## 🤝 Contributing

Issues and Pull Requests are welcome!

1. Fork the Project
2. Create your Feature Branch (`git checkout -b feat/amazing-feature`)
3. Commit your Changes (conventional commits)
4. Push to the Branch
5. Open a Pull Request

## 📄 License

MIT License - Free to use, modify, and distribute.

## 🙏 Thanks

- [Logos](https://github.com/maciejhirsz/logos) - High-performance lexer generator
- [Chumsky](https://github.com/zesterer/chumsky) - Parser combinator library
- [Ariadne](https://github.com/zesterer/ariadne) - Beautiful error reporting
- [Monaco Editor](https://microsoft.github.io/monaco-editor/) - Web editor
