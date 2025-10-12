## PyHyeon: a subset language of Python

This repository includes a lexer and a parser. The interpreter, compiler, and VM are under development.

```py
def add(a, b):
  return a + b

x = 5
y = add(x, 1)
```

### Requirements
- Rust toolchain with the 2024 edition (Rust 1.84+)

### Run
```bash
cargo run
```
Reads `./test.pyh` and prints tokens and the AST to the terminal.