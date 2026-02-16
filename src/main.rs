mod ast;
mod codegen;
mod error;
mod lexer;
mod loader;
mod parser;
mod resolver;
mod token;
mod typechecker;

use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process;

use codegen::CodeGen;
use error::{CompilerError, Diagnostic, LustcResult, SourceMap};
use lexer::Lexer;
use loader::ModuleLoader;
use parser::Parser;
use resolver::Resolver;
use typechecker::TypeChecker;

/// Compile a single source string (no import resolution). Used by tests.
#[allow(dead_code)]
fn compile(source: &str) -> LustcResult<String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let decls = parser.parse_program()?;

    let resolver = Resolver::new();
    resolver
        .resolve(&decls)
        .map_err(CompilerError::Multiple)?;

    let checker = TypeChecker::new();
    checker
        .check(&decls)
        .map_err(CompilerError::Multiple)?;

    let mut codegen = CodeGen::new();
    let raw_output = codegen.generate(&decls)?;

    // Post-process with rustfmt for cargo fmt-equivalent output
    Ok(format_with_rustfmt(&raw_output))
}

/// Compile a file with full import resolution.
fn compile_file(path: &Path) -> Result<String, (CompilerError, Vec<(String, String)>)> {
    let mut loader = ModuleLoader::new(path);
    let decls = loader
        .load_program(path)
        .map_err(|e| (e, loader.file_sources.clone()))?;

    let resolver = Resolver::new();
    resolver
        .resolve(&decls)
        .map_err(|errs| (CompilerError::Multiple(errs), loader.file_sources.clone()))?;

    let checker = TypeChecker::new();
    checker
        .check(&decls)
        .map_err(|errs| (CompilerError::Multiple(errs), loader.file_sources.clone()))?;

    let mut codegen = CodeGen::new();
    let raw_output = codegen
        .generate(&decls)
        .map_err(|e| (e, loader.file_sources.clone()))?;

    Ok(format_with_rustfmt(&raw_output))
}

fn format_with_rustfmt(code: &str) -> String {
    let mut child = match process::Command::new("rustfmt")
        .arg("--edition")
        .arg("2021")
        .stdin(process::Stdio::piped())
        .stdout(process::Stdio::piped())
        .stderr(process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return code.to_string(), // rustfmt not available, fall back
    };

    if let Some(ref mut stdin) = child.stdin {
        if stdin.write_all(code.as_bytes()).is_err() {
            return code.to_string();
        }
    }
    // Close stdin so rustfmt can process
    drop(child.stdin.take());

    match child.wait_with_output() {
        Ok(output) if output.status.success() => {
            String::from_utf8(output.stdout).unwrap_or_else(|_| code.to_string())
        }
        _ => code.to_string(), // Fall back on failure
    }
}

fn print_error(error: &CompilerError, source: &str, filename: &str) {
    let source_map = SourceMap::new(source);
    match error {
        CompilerError::Multiple(errors) => {
            for (i, err) in errors.iter().enumerate() {
                if i > 0 {
                    eprintln!();
                }
                let diag = Diagnostic::new(err, &source_map, filename);
                eprint!("{}", diag);
            }
        }
        _ => {
            let diag = Diagnostic::new(error, &source_map, filename);
            eprint!("{}", diag);
        }
    }
}

fn print_error_with_sources(error: &CompilerError, file_sources: &[(String, String)]) {
    match error {
        CompilerError::ModuleError { msg } => {
            eprintln!("module error: {}", msg);
        }
        _ => {
            // Use the first file source as default
            if let Some((filename, source)) = file_sources.first() {
                print_error(error, source, filename);
            } else {
                eprintln!("{}", error);
            }
        }
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: lustc <input.lean> [-o <output.rs>]");
        process::exit(1);
    }

    let input_path = &args[1];
    let output_path = if args.len() >= 4 && args[2] == "-o" {
        args[3].clone()
    } else {
        // Default: replace .lean with .rs
        let p = Path::new(input_path);
        p.with_extension("rs").to_string_lossy().to_string()
    };

    let path = Path::new(input_path);
    match compile_file(path) {
        Ok(rust_code) => {
            if let Err(e) = fs::write(&output_path, &rust_code) {
                eprintln!("Error writing {}: {}", output_path, e);
                process::exit(1);
            }
            println!("Compiled {} -> {}", input_path, output_path);
        }
        Err((e, file_sources)) => {
            print_error_with_sources(&e, &file_sources);
            process::exit(1);
        }
    }
}
