# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What is lustc?

A Lean4 subset to Rust source-to-source compiler written in Rust. It translates `.lean` files into `.rs` files that can be compiled with `rustc`.

## Build & Test Commands

```sh
cargo build              # Build the compiler
cargo test               # Run all tests (unit + integration)
cargo test test_name     # Run a specific test by name
cargo clippy             # Lint — must produce zero warnings
cargo run -- input.lean -o output.rs  # Compile a Lean file
```

Integration tests (`tests/integration_test.rs`) invoke the compiled `lustc` binary via `Command`, write temp `.lean` files, compile them, and optionally run the resulting Rust binary to check output.

## Compiler Pipeline

```
Lean4 source → Lexer → Parser → ModuleLoader (imports) → Resolver → TypeChecker → CodeGen → rustfmt → Rust source
```

Each stage lives in its own module under `src/`:

| Module | Role |
|---|---|
| `token.rs` | `TokenKind` enum — all tokens including keywords |
| `lexer.rs` | Tokenization with indentation-based Indent/Dedent tokens |
| `ast.rs` | AST types: `Decl`, `Expr`, `Pattern`, `Type`, `ModulePath` |
| `parser.rs` | Recursive descent parser producing `Vec<Decl>` |
| `loader.rs` | Multi-file module loading, circular import detection |
| `resolver.rs` | Name resolution with scope stack; validates all variables are defined |
| `typechecker.rs` | Type inference/checking with `Ty` internal type representation |
| `codegen.rs` | Rust code generation with dead code elimination (DCE) |
| `error.rs` | `CompilerError` enum, `Span`, `Diagnostic` (rich error display) |
| `main.rs` | CLI entry point; `compile()` for single-source, `compile_file()` for multi-file |

## Key Architecture Details

- **Two compile paths**: `compile()` (single source string, used by tests) and `compile_file()` (file-based with import resolution, used by CLI).
- **DCE**: `codegen.rs` runs a reachability analysis from `main`/`#eval` entry points. Only reachable declarations are emitted.
- **Name translation**: Lean's `camelCase` → Rust's `snake_case` for functions; `PascalCase` for enum variants. Qualified names like `Color.red` → `Color::Red`, `Math.square` → `math::square`.
- **Module system**: `import Foo` loads `Foo.lean` and wraps declarations in `Decl::Namespace`. `open Foo` makes members accessible unqualified. No Rust `use` statements are generated — resolution happens in codegen's `translate_var()` to avoid rustfmt reordering issues.
- **All output passes through `rustfmt`** — the compiler generates structurally correct but unformatted Rust, then pipes through `rustfmt --edition 2021`.

## Lean4 → Rust Mappings

- `Nat` → `u64`, `Int` → `i64`, `Bool` → `bool`, `String` → `String`
- `IO.println` → `println!`, `toString` → `.to_string()`
- `inductive` → `enum`, `structure` → `struct`
- `#eval expr` → `fn main() { println!("{}", expr); }`
- `namespace Foo ... end Foo` → `mod foo { pub ... }`
- Tuple field access is 1-indexed in Lean, 0-indexed in Rust (`p.1` → `p.0`)
