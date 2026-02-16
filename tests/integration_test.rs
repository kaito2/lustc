use std::process::Command;

fn compile_lean_fail(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let lean_path = dir.path().join("test.lean");
    let rs_path = dir.path().join("test.rs");

    std::fs::write(&lean_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        !output.status.success(),
        "lustc should have failed but succeeded"
    );

    String::from_utf8_lossy(&output.stderr).to_string()
}

fn compile_lean(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let lean_path = dir.path().join("test.lean");
    let rs_path = dir.path().join("test.rs");

    std::fs::write(&lean_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        output.status.success(),
        "lustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read_to_string(&rs_path).unwrap()
}

fn compile_and_run(source: &str) -> String {
    let dir = tempfile::tempdir().unwrap();
    let lean_path = dir.path().join("test.lean");
    let rs_path = dir.path().join("test.rs");
    let bin_path = dir.path().join("test_bin");

    std::fs::write(&lean_path, source).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        output.status.success(),
        "lustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("rustc")
        .arg(rs_path.to_str().unwrap())
        .arg("-o")
        .arg(bin_path.to_str().unwrap())
        .output()
        .expect("failed to run rustc");

    assert!(
        output.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(bin_path.to_str().unwrap())
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "binary failed");

    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn test_hello_world() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  IO.println "Hello, World!"
"#,
    );
    assert_eq!(output.trim(), "Hello, World!");
}

#[test]
fn test_arithmetic_functions() {
    let output = compile_and_run(
        r#"def add (x : Nat) (y : Nat) : Nat := x + y

def main : IO Unit := do
  IO.println (toString (add 3 4))
"#,
    );
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_factorial() {
    let output = compile_and_run(
        r#"def factorial (n : Nat) : Nat :=
  if n == 0 then 1 else n * factorial (n - 1)

def main : IO Unit := do
  IO.println (toString (factorial 5))
"#,
    );
    assert_eq!(output.trim(), "120");
}

#[test]
fn test_inductive_type() {
    let rust = compile_lean(
        r#"inductive Color where
  | red
  | green
  | blue
"#,
    );
    assert!(rust.contains("#[derive(Debug, Clone, PartialEq)]"));
    assert!(rust.contains("enum Color"));
    assert!(rust.contains("Red"));
    assert!(rust.contains("Green"));
    assert!(rust.contains("Blue"));
}

#[test]
fn test_eval() {
    let output = compile_and_run("#eval 2 + 3 * 4");
    assert_eq!(output.trim(), "14");
}

#[test]
fn test_if_else() {
    let output = compile_and_run(
        r#"def max (a : Nat) (b : Nat) : Nat :=
  if a >= b then a else b

def main : IO Unit := do
  IO.println (toString (max 10 20))
"#,
    );
    assert_eq!(output.trim(), "20");
}

#[test]
fn test_saturating_subtraction() {
    let output = compile_and_run(
        r#"def pred (n : Nat) : Nat := n - 1

def main : IO Unit := do
  IO.println (toString (pred 0))
  IO.println (toString (pred 5))
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "0");
    assert_eq!(lines[1], "4");
}

#[test]
fn test_multiple_println() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  IO.println "first"
  IO.println "second"
  IO.println "third"
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "first");
    assert_eq!(lines[1], "second");
    assert_eq!(lines[2], "third");
}

#[test]
fn test_string_functions() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  IO.println (toString 42)
"#,
    );
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_pattern_match_function_def() {
    let output = compile_and_run(
        r#"def factorial : Nat → Nat
  | 0 => 1
  | n + 1 => (n + 1) * factorial n

def main : IO Unit := do
  IO.println (toString (factorial 5))
"#,
    );
    assert_eq!(output.trim(), "120");
}

#[test]
fn test_let_binding_in_do() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  let x := 10
  let y := 20
  IO.println (toString (x + y))
"#,
    );
    assert_eq!(output.trim(), "30");
}

#[test]
fn test_lambda() {
    let output = compile_and_run(
        r#"def apply (f : Nat → Nat) (x : Nat) : Nat := f x

#eval apply (fun n => n + 1) 5
"#,
    );
    assert_eq!(output.trim(), "6");
}

#[test]
fn test_undefined_variable_fails() {
    let stderr = compile_lean_fail("def f (x : Nat) : Nat := y + 1\n");
    assert!(
        stderr.contains("undefined variable `y`"),
        "expected undefined variable error, got: {}",
        stderr
    );
}

#[test]
fn test_multiple_parse_errors_reported() {
    let stderr = compile_lean_fail("def := 42\ndef := 99\n");
    // Should contain at least two "error:" lines
    let error_count = stderr.matches("error:").count();
    assert!(
        error_count >= 2,
        "expected at least 2 errors, got {}: {}",
        error_count,
        stderr
    );
}

#[test]
fn test_rich_error_display() {
    let stderr = compile_lean_fail("def f (x : Nat) : Nat := y + 1\n");
    // Should contain the source line and caret
    assert!(
        stderr.contains("-->"),
        "expected rich diagnostic with -->, got: {}",
        stderr
    );
    assert!(
        stderr.contains("^"),
        "expected caret in diagnostic, got: {}",
        stderr
    );
}

// --- v0.3.0: Structure definition ---

#[test]
fn test_structure_definition() {
    let rust = compile_lean(
        r#"structure Point where
  x : Nat
  y : Nat
"#,
    );
    assert!(rust.contains("#[derive(Debug, Clone, PartialEq)]"));
    assert!(rust.contains("struct Point"));
    assert!(rust.contains("x: u64"));
    assert!(rust.contains("y: u64"));
}

#[test]
fn test_structure_constructor_and_field_access() {
    let output = compile_and_run(
        r#"structure Point where
  x : Nat
  y : Nat

def main : IO Unit := do
  let p := Point.mk 10 20
  IO.println (toString (Point.x p))
  IO.println (toString (Point.y p))
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
}

// --- v0.3.0: Tuple support ---

#[test]
fn test_tuple_creation_and_access() {
    let output = compile_and_run(
        r#"def fst (p : Nat × Nat) : Nat := p.1
def snd (p : Nat × Nat) : Nat := p.2

def main : IO Unit := do
  IO.println (toString (fst (10, 20)))
  IO.println (toString (snd (10, 20)))
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
}

// --- v0.3.0: String interpolation ---

#[test]
fn test_string_interpolation() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  let name := "World"
  IO.println s!"Hello, {name}!"
"#,
    );
    assert_eq!(output.trim(), "Hello, World!");
}

#[test]
fn test_string_interpolation_with_expr() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  let x := 3
  let y := 4
  IO.println s!"{x} + {y} = {x + y}"
"#,
    );
    assert_eq!(output.trim(), "3 + 4 = 7");
}

// --- v0.3.0: Where clause ---

#[test]
fn test_where_clause() {
    let output = compile_and_run(
        r#"def circleArea (r : Nat) : Nat :=
  pi * r * r
  where pi := 3

def main : IO Unit := do
  IO.println (toString (circleArea 5))
"#,
    );
    assert_eq!(output.trim(), "75");
}

// --- v0.3.0: List/Option type mapping ---

#[test]
fn test_option_some_none() {
    let output = compile_and_run(
        r#"def showOpt (o : Option Nat) : String :=
  match o with
  | Option.none => "nothing"
  | Option.some x => s!"got {x}"

def main : IO Unit := do
  IO.println (showOpt (Option.some 42))
  IO.println (showOpt Option.none)
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "got 42");
    assert_eq!(lines[1], "nothing");
}

// --- v0.3.0: Constructor enhancement ---

#[test]
fn test_inductive_with_type_app_fields() {
    let rust = compile_lean(
        r#"inductive MyList where
  | nil
  | cons : Nat → MyList → MyList
"#,
    );
    assert!(rust.contains("enum MyList"));
    assert!(rust.contains("Nil"));
    assert!(rust.contains("Cons(u64, MyList)"));
}

// --- v0.4.0: Type checker ---

#[test]
fn test_type_error_arithmetic_on_bool() {
    let stderr = compile_lean_fail("def f (x : Bool) : Nat := x + 1\n");
    assert!(
        stderr.contains("type error"),
        "expected type error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("numeric"),
        "expected numeric type error, got: {}",
        stderr
    );
}

#[test]
fn test_type_error_if_branch_mismatch() {
    let stderr = compile_lean_fail("def f (x : Nat) : Nat := if x == 0 then 1 else true\n");
    assert!(
        stderr.contains("type error"),
        "expected type error, got: {}",
        stderr
    );
    assert!(
        stderr.contains("type mismatch"),
        "expected type mismatch, got: {}",
        stderr
    );
}

#[test]
fn test_type_error_return_type_mismatch() {
    let stderr = compile_lean_fail("def f (x : Nat) : String := x + 1\n");
    assert!(
        stderr.contains("type error"),
        "expected type error, got: {}",
        stderr
    );
}

#[test]
fn test_codegen_no_redundant_tostring_on_string() {
    let rust = compile_lean(
        r#"def main : IO Unit := do
  IO.println (toString "hello")
"#,
    );
    // Should NOT contain .to_string() since "hello" is already a String
    assert!(
        !rust.contains(".to_string()"),
        "expected no .to_string() on string literal, got: {}",
        rust
    );
}

#[test]
fn test_codegen_removes_redundant_parens() {
    let rust = compile_lean(
        r#"def f (x : Nat) : Nat := (x)
"#,
    );
    // The body should be just `x`, not `(x)`
    assert!(
        !rust.contains("(x)"),
        "expected no redundant parens around simple var, got: {}",
        rust
    );
}

// --- v0.5.0: Generated code quality ---

#[test]
fn test_snake_case_function_names() {
    let output = compile_and_run(
        r#"def circleArea (r : Nat) : Nat := r * r * 3

def main : IO Unit := do
  IO.println (toString (circleArea 5))
"#,
    );
    assert_eq!(output.trim(), "75");
}

#[test]
fn test_snake_case_in_generated_code() {
    let rust = compile_lean(
        r#"def circleArea (r : Nat) : Nat := r * r * 3

def main : IO Unit := do
  IO.println (toString (circleArea 5))
"#,
    );
    // Function should be renamed to snake_case
    assert!(
        rust.contains("circle_area"),
        "expected snake_case function name, got: {}",
        rust
    );
    assert!(
        !rust.contains("circleArea"),
        "expected no camelCase, got: {}",
        rust
    );
}

#[test]
fn test_dead_code_elimination() {
    let rust = compile_lean(
        r#"def unused (x : Nat) : Nat := x + 1

def helper (x : Nat) : Nat := x * 2

def main : IO Unit := do
  IO.println (toString (helper 5))
"#,
    );
    // `unused` should be eliminated
    assert!(
        !rust.contains("fn unused"),
        "expected dead code removed, got: {}",
        rust
    );
    // `helper` should be present
    assert!(
        rust.contains("helper"),
        "expected reachable function present, got: {}",
        rust
    );
}

#[test]
fn test_dead_code_inductive_reachable() {
    let output = compile_and_run(
        r#"inductive Color where
  | red
  | green
  | blue

def colorName (c : Color) : String :=
  match c with
  | Color.red => "Red"
  | Color.green => "Green"
  | Color.blue => "Blue"

def unusedHelper (x : Nat) : Nat := x

def main : IO Unit := do
  IO.println (colorName Color.green)
"#,
    );
    assert_eq!(output.trim(), "Green");
}

#[test]
fn test_rustfmt_formatted_output() {
    let rust = compile_lean(
        r#"def add (x : Nat) (y : Nat) : Nat := x + y

def main : IO Unit := do
  IO.println (toString (add 3 4))
"#,
    );
    // rustfmt should produce well-formatted output
    // At minimum, it should have proper newlines and indentation
    assert!(
        rust.contains("fn main()"),
        "expected formatted main function, got: {}",
        rust
    );
}

// --- Multi-file helpers ---

fn compile_and_run_files(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();

    // Write all files
    for (name, content) in files {
        let file_path = dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, content).unwrap();
    }

    // The last file is the entry point
    let (entry_name, _) = files.last().unwrap();
    let lean_path = dir.path().join(entry_name);
    let rs_path = dir.path().join("output.rs");
    let bin_path = dir.path().join("test_bin");

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        output.status.success(),
        "lustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new("rustc")
        .arg(rs_path.to_str().unwrap())
        .arg("-o")
        .arg(bin_path.to_str().unwrap())
        .output()
        .expect("failed to run rustc");

    assert!(
        output.status.success(),
        "rustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let output = Command::new(bin_path.to_str().unwrap())
        .output()
        .expect("failed to run binary");

    assert!(output.status.success(), "binary failed");

    String::from_utf8(output.stdout).unwrap()
}

fn compile_files(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let file_path = dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, content).unwrap();
    }

    let (entry_name, _) = files.last().unwrap();
    let lean_path = dir.path().join(entry_name);
    let rs_path = dir.path().join("output.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        output.status.success(),
        "lustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    std::fs::read_to_string(&rs_path).unwrap()
}

fn compile_files_fail(files: &[(&str, &str)]) -> String {
    let dir = tempfile::tempdir().unwrap();

    for (name, content) in files {
        let file_path = dir.path().join(name);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&file_path, content).unwrap();
    }

    let (entry_name, _) = files.last().unwrap();
    let lean_path = dir.path().join(entry_name);
    let rs_path = dir.path().join("output.rs");

    let output = Command::new(env!("CARGO_BIN_EXE_lustc"))
        .arg(lean_path.to_str().unwrap())
        .arg("-o")
        .arg(rs_path.to_str().unwrap())
        .output()
        .expect("failed to run lustc");

    assert!(
        !output.status.success(),
        "lustc should have failed but succeeded"
    );

    String::from_utf8_lossy(&output.stderr).to_string()
}

// --- v0.3.0: Cross-feature integration ---

#[test]
fn test_cross_feature_struct_where_interpolation() {
    let output = compile_and_run(
        r#"structure Point where
  x : Nat
  y : Nat

def describe (p : Point) : String :=
  s!"({px}, {py})"
  where px := Point.x p
  where py := Point.y p

def main : IO Unit := do
  let p := Point.mk 3 7
  IO.println (describe p)
"#,
    );
    assert_eq!(output.trim(), "(3, 7)");
}

// --- v0.6.0: Module System ---

#[test]
fn test_basic_import() {
    let output = compile_and_run_files(&[
        (
            "Helpers.lean",
            r#"def answer : Nat := 42
"#,
        ),
        (
            "main.lean",
            r#"import Helpers

def main : IO Unit := do
  IO.println (toString (Helpers.answer))
"#,
        ),
    ]);
    assert_eq!(output.trim(), "42");
}

#[test]
fn test_nested_import() {
    let output = compile_and_run_files(&[
        (
            "Utils/Math.lean",
            r#"def add (x : Nat) (y : Nat) : Nat := x + y
"#,
        ),
        (
            "main.lean",
            r#"import Utils.Math

def main : IO Unit := do
  IO.println (toString (Math.add 3 4))
"#,
        ),
    ]);
    assert_eq!(output.trim(), "7");
}

#[test]
fn test_namespace() {
    let output = compile_and_run(
        r#"namespace Math
def square (x : Nat) : Nat := x * x
end Math

def main : IO Unit := do
  IO.println (toString (Math.square 5))
"#,
    );
    assert_eq!(output.trim(), "25");
}

#[test]
fn test_namespace_with_open() {
    let output = compile_and_run(
        r#"namespace Math
def square (x : Nat) : Nat := x * x
end Math

open Math

def main : IO Unit := do
  IO.println (toString (square 5))
"#,
    );
    assert_eq!(output.trim(), "25");
}

#[test]
fn test_import_generated_code() {
    let rust = compile_files(&[
        (
            "Helpers.lean",
            r#"def greet : Nat := 99
"#,
        ),
        (
            "main.lean",
            r#"import Helpers

def main : IO Unit := do
  IO.println (toString (Helpers.greet))
"#,
        ),
    ]);
    assert!(
        rust.contains("mod helpers"),
        "expected mod block, got: {}",
        rust
    );
    assert!(
        rust.contains("pub fn"),
        "expected pub fn in mod, got: {}",
        rust
    );
}

#[test]
fn test_circular_import_error() {
    let stderr = compile_files_fail(&[
        ("A.lean", "import B\n"),
        ("B.lean", "import A\n"),
        ("main.lean", "import A\n#eval 1\n"),
    ]);
    assert!(
        stderr.contains("circular import"),
        "expected circular import error, got: {}",
        stderr
    );
}

#[test]
fn test_missing_import_error() {
    let stderr = compile_files_fail(&[("main.lean", "import NonExistent\n#eval 1\n")]);
    assert!(
        stderr.contains("not found"),
        "expected module not found error, got: {}",
        stderr
    );
}

#[test]
fn test_namespace_inductive_type() {
    let output = compile_and_run(
        r#"namespace Shapes
inductive Color where
  | red
  | green
  | blue
end Shapes

def main : IO Unit := do
  IO.println "ok"
"#,
    );
    assert_eq!(output.trim(), "ok");
}

#[test]
fn test_cross_feature_import_namespace_struct() {
    let output = compile_and_run_files(&[
        (
            "Geometry.lean",
            r#"def double (x : Nat) : Nat := x * 2
"#,
        ),
        (
            "main.lean",
            r#"import Geometry

def main : IO Unit := do
  IO.println (toString (Geometry.double 21))
"#,
        ),
    ]);
    assert_eq!(output.trim(), "42");
}

// --- v0.7.0: Comprehensive tests ---

#[test]
fn test_empty_file() {
    let rust = compile_lean("");
    // Empty file should produce empty or minimal Rust output
    assert!(
        rust.trim().is_empty() || !rust.contains("fn main"),
        "empty file should produce no main, got: {}",
        rust
    );
}

#[test]
fn test_comments_only_file() {
    let rust = compile_lean("-- This is a comment\n-- Another comment\n");
    assert!(
        rust.trim().is_empty() || !rust.contains("fn main"),
        "comments-only file should produce no main, got: {}",
        rust
    );
}

#[test]
fn test_multiple_eval() {
    let output = compile_and_run("#eval 10\n#eval 20\n#eval 30\n");
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "10");
    assert_eq!(lines[1], "20");
    assert_eq!(lines[2], "30");
}

#[test]
fn test_deeply_nested_expression() {
    let output = compile_and_run("#eval ((((1 + 2) * 3) + 4) * 5)\n");
    // ((1+2)*3+4)*5 = (3*3+4)*5 = (9+4)*5 = 13*5 = 65
    assert_eq!(output.trim(), "65");
}

#[test]
fn test_nested_if_else() {
    let output = compile_and_run(
        r#"def classify (n : Nat) : String :=
  if n == 0 then "zero"
  else if n == 1 then "one"
  else if n == 2 then "two"
  else "many"

def main : IO Unit := do
  IO.println (classify 0)
  IO.println (classify 1)
  IO.println (classify 2)
  IO.println (classify 99)
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "zero");
    assert_eq!(lines[1], "one");
    assert_eq!(lines[2], "two");
    assert_eq!(lines[3], "many");
}

#[test]
fn test_list_cons_construction() {
    let rust = compile_lean(
        r#"def myList : List Nat := List.cons 1 (List.cons 2 List.nil)
"#,
    );
    assert!(
        rust.contains("vec![]"),
        "expected vec![] for List.nil, got: {}",
        rust
    );
    assert!(
        rust.contains("insert(0,"),
        "expected insert for List.cons, got: {}",
        rust
    );
}

#[test]
fn test_import_then_open() {
    let output = compile_and_run_files(&[
        (
            "MathLib.lean",
            r#"def add (x : Nat) (y : Nat) : Nat := x + y
"#,
        ),
        (
            "main.lean",
            r#"import MathLib
open MathLib

def main : IO Unit := do
  IO.println (toString (add 10 20))
"#,
        ),
    ]);
    assert_eq!(output.trim(), "30");
}

#[test]
fn test_bool_literals_and_logic() {
    let output = compile_and_run(
        r#"def main : IO Unit := do
  IO.println (toString (true && false))
  IO.println (toString (true || false))
  IO.println (toString (!true))
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "false");
    assert_eq!(lines[1], "true");
    assert_eq!(lines[2], "false");
}

#[test]
fn test_tuple_pattern_match() {
    let output = compile_and_run(
        r#"def swap (p : Nat × Nat) : Nat × Nat :=
  match p with
  | (a, b) => (b, a)

def main : IO Unit := do
  let r := swap (1, 2)
  IO.println (toString r.1)
  IO.println (toString r.2)
"#,
    );
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines[0], "2");
    assert_eq!(lines[1], "1");
}

#[test]
fn test_unterminated_string_error() {
    let stderr = compile_lean_fail("#eval \"hello\n");
    assert!(
        stderr.contains("unterminated string"),
        "expected unterminated string error, got: {}",
        stderr
    );
}

#[test]
fn test_unexpected_token_error() {
    let stderr = compile_lean_fail("def + 42\n");
    assert!(
        stderr.contains("error"),
        "expected error for unexpected token, got: {}",
        stderr
    );
}

#[test]
fn test_namespace_end_mismatch_error() {
    let stderr = compile_lean_fail(
        r#"namespace Foo
def x : Nat := 1
end Bar
"#,
    );
    assert!(
        stderr.contains("end Foo") || stderr.contains("end Bar"),
        "expected namespace end mismatch error, got: {}",
        stderr
    );
}
