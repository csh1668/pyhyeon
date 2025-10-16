## PyHyeon: a subset language of Python

This repository implements a small Python subset language (lexer, parser, semantic analyzer), an interpreter, a bytecode compiler, and a simple VM.

```py
def add(a, b):
  return a + b

x = 5
y = add(x, 1)
```

### Requirements
- Rust toolchain with the 2024 edition (Rust 1.84+)

### Build
```bash
cargo build
```

### CLI
- `run`: parse → analyze → execute (interpreter by default)
- `compile`: parse → analyze → compile to bytecode file (`.pyhb`)
- `exec`: execute a compiled bytecode file on the VM

#### Examples
```bash
# Run with interpreter
cargo run -- run ./test.pyh

# Run with VM
cargo run -- run ./test.pyh --engine=vm

# Compile to bytecode (out.pyhb)
cargo run -- compile ./test.pyh -o out.pyhb

# Execute bytecode on VM
cargo run -- exec ./out.pyhb
```