# lustc

A Lean4 subset to Rust source-to-source compiler, written in Rust.

## Supported Lean4 Subset

- `def` function definitions (`:=` form and pattern match form)
- `let` bindings, `if/then/else`
- Basic types: `Nat`→`u64`, `Int`→`i64`, `Bool`→`bool`, `String`→`String`, `Unit`→`()`
- Arithmetic, comparison, and logical operators
- Pattern matching (`match ... with`), successor patterns (`n + 1`)
- Simple inductive types (`inductive ... where`) → Rust `enum`
- `structure` definitions → Rust `struct` (with `.mk` constructor and field accessors)
- `do` blocks (IO), `IO.println` → `println!`
- `#eval` → `main` with `println!`
- `toString` → `.to_string()`
- Unicode arrows (`→`, `←`) and ASCII equivalents (`->`, `<-`)
- Lambda expressions (`fun x => ...`)
- Tuples `(a, b)` and tuple types `Nat × Nat`
- `where` clause local definitions
- String interpolation `s!"..."` → `format!(...)`
- `List` / `Option` type mapping → `Vec<T>` / `Option<T>`
- Type checker with type mismatch error reporting
- Automatic `camelCase` → `snake_case` conversion for Rust conventions
- Dead code elimination (unused functions/types removed)
- `rustfmt`-formatted output

## Installation

```sh
cargo install --path .
```

## Usage

```sh
lustc <input.lean> [-o <output.rs>]
```

If `-o` is omitted, the output file defaults to the input filename with a `.rs` extension.

## Examples

### Hello World

```lean
-- examples/hello.lean
def main : IO Unit := do
  IO.println "Hello, World!"
```

```sh
lustc examples/hello.lean -o /tmp/hello.rs
rustc /tmp/hello.rs -o /tmp/hello
/tmp/hello
# => Hello, World!
```

### Factorial

```lean
-- examples/factorial.lean
def factorial (n : Nat) : Nat :=
  if n == 0 then 1 else n * factorial (n - 1)

def main : IO Unit := do
  IO.println (toString (factorial 5))
```

### Inductive Types

```lean
-- examples/color.lean
inductive Color where
  | red
  | green
  | blue

def colorName (c : Color) : String :=
  match c with
  | Color.red => "Red"
  | Color.green => "Green"
  | Color.blue => "Blue"

def main : IO Unit := do
  IO.println (colorName Color.red)
```

### Structures

```lean
structure Point where
  x : Nat
  y : Nat

def main : IO Unit := do
  let p := Point.mk 10 20
  IO.println (toString (Point.x p))
```

### Tuples and String Interpolation

```lean
def main : IO Unit := do
  let p := (3, 4)
  IO.println s!"x = {p.1}, y = {p.2}"
```

### Where Clause

```lean
def circleArea (r : Nat) : Nat :=
  pi * r * r
  where pi := 3
```

### Option Type

```lean
def showOpt (o : Option Nat) : String :=
  match o with
  | Option.none => "nothing"
  | Option.some x => s!"got {x}"
```

## Compiler Pipeline

```
Lean4 source → Lexer → Tokens → Parser → AST → Resolver → TypeChecker → CodeGen → Rust source
```

## Running Tests

```sh
cargo test
```

## Roadmap

See [ROADMAP.md](ROADMAP.md) for planned features and milestones.

## License

MIT
