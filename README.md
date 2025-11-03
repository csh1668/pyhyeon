# Pyhyeon

<div align="center">

[![Rust](https://img.shields.io/badge/rust-1.84%2B-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Playground](https://img.shields.io/badge/try-playground-brightgreen.svg)](https://csh1668.github.io/pyhyeon/)

**A subset language of Python**

</div>

---

## 🎯 Overview

Pyhyeon is a toy programming language implementing Python's core syntax. It provides a complete language implementation pipeline: Lexer, Parser, Semantic Analyzer, Bytecode Compiler, and Stack-based VM.

### ✨ Features

- ✅ **Complete Compiler Pipeline**: Lexing → Parsing → Semantic Analysis → Bytecode VM Execution
- ✅ **Stack-based Virtual Machine**: Efficient bytecode execution engine
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

# Lists
nums = [1, 2, 3, 4, 5]
nums.append(6)
for n in nums:
  print(n)

# Dicts
person = {"name": "Alice", "age": 30}
print(person["name"])
for key in person:
  print(key)
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
# Run a program (compiles and executes with VM)
cargo run --release --bin pyhyeon -- run test.pyh

# Compile to bytecode
cargo run --release --bin pyhyeon -- compile test.pyh -o test.pyhb

# Execute compiled bytecode
cargo run --release --bin pyhyeon -- exec test.pyhb
```

## 📚 Language Features

### Data Types
- `int` - 64-bit signed integer
- `bool` - Boolean (`True`, `False`)
- `str` - String literals with `"` or `'`
  - Escape sequences: `\n`, `\t`, `\r`, `\\`, `\"`, `\'`
  - Unicode support (UTF-8)
  - Methods: `upper()`, `lower()`, `strip()`, `split()`, `join()`, `replace()`, etc.
- `list` - Mutable list `[1, 2, 3]`
  - Indexing: `x[0]`, `x[-1]`
  - Methods: `append()`, `pop()`, `extend()`, `insert()`, `remove()`, `reverse()`, `sort()`, `clear()`, `index()`, `count()`
  - Iterable in `for` loops
- `dict` - Mutable dictionary `{"a": 1, "b": 2}`
  - Indexing: `d["key"]`
  - Methods: `get()`, `keys()`, `values()`, `clear()`
  - Iterable in `for` loops (iterates over keys)
- `None` - Null value

### Operators
- **Arithmetic**: `+`, `-`, `*`, `//` (floor division), `%`
  - String concatenation: `"hello" + " world"`
  - String repetition: `"ab" * 3` → `"ababab"`
- **Comparison**: `==`, `!=`, `<`, `>`, `<=`, `>=`
  - Lexicographic string comparison supported
- **Logical**: `and`, `or`, `not` (with short-circuit evaluation)
- **Unary**: `+`, `-`, `not`

### Control Structures
- `if` / `elif` / `else`
- `while` loops
- `for` loops with iterables (lists, dicts, ranges)
- Function definitions (`def`) with recursion support

### Built-in Functions
- `print(x)` - Output a value
- `input()` - Read a line from stdin (returns string)
- `int(x)` - Convert to integer
- `bool(x)` - Convert to boolean
- `str(x)` - Convert to string
- `len(s)` - Get length (strings, lists, dicts)
- `range(n)` - Create a range iterator for `for` loops

## 🏗️ Architecture

```
Source Code (.pyh)
    ↓
Lexer (logos) → Token Stream
    ↓
Parser (chumsky) → AST
    ↓
Semantic Analyzer → Validated AST
    ↓
Compiler → Bytecode (.pyhb)
    ↓
VM → Execute
```

## 🧪 Testing

```bash
# Run all tests (unit + E2E)
cargo test

# Run E2E tests only
cargo test --test e2e_tests
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
