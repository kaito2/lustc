# lustc Roadmap

## v0.2.0 — Error Experience ✅

- Source-location-aware error messages (line number, source line display)
- Parser error recovery (report multiple errors instead of stopping at the first)
- Undefined variable/function detection (add a simple name resolution pass)

## v0.3.0 — Language Feature Expansion ✅

- `structure` definitions → Rust `struct` (with `Name.mk` constructor and `Name.field` accessor)
- Constructors with fields (`| cons : Nat → List Nat → List Nat`) — type application in field types
- Tuples `(a, b)` and tuple types `Nat × Nat`, tuple pattern matching, 1-indexed → 0-indexed field access
- `where` clause local definitions (`def f ... where helper := ...`) — desugared to `let`
- String interpolation `s!"..."` → `format!(...)` with embedded expression parsing
- `List` / `Option` type mapping → `Vec<T>` / `Option<T>` with builtin constructors and pattern matching

## v0.4.0 — Type Checker

- Simple type inference / type checking pass
- Type mismatch error reporting (catch errors before code generation)
- Remove unnecessary `.to_string()` and redundant parentheses from generated code

## v0.5.0 — Generated Code Quality

- `cargo fmt`-equivalent formatted output
- Automatic `snake_case` conversion (Lean's `camelCase` function names → Rust convention)
- Remove unnecessary `#[allow(unused)]` annotations
- Dead code detection and elimination

## v0.6.0 — Module System

- `import` / `open` → Rust `mod` / `use`
- Multi-file input support
- Namespace resolution

## v1.0.0 — Stable Release

- Comprehensive test suite (validate against a subset of Lean4 Mathlib)
- CI/CD (GitHub Actions: test / clippy / fmt)
- Distribution via `cargo install lustc`
- Documentation site or detailed reference guide
