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
