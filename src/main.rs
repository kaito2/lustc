mod ast;
mod codegen;
mod error;
mod lexer;
mod parser;
mod resolver;
mod token;

use std::env;
use std::fs;
use std::path::Path;
use std::process;

use codegen::CodeGen;
use error::{CompilerError, Diagnostic, LustcResult, SourceMap};
use lexer::Lexer;
use parser::Parser;
use resolver::Resolver;

fn compile(source: &str) -> LustcResult<String> {
    let mut lexer = Lexer::new(source);
    let tokens = lexer.tokenize()?;
    let mut parser = Parser::new(tokens);
    let decls = parser.parse_program()?;

    let resolver = Resolver::new();
    resolver
        .resolve(&decls)
        .map_err(CompilerError::Multiple)?;

    let mut codegen = CodeGen::new();
    codegen.generate(&decls)
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

    let source = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Error reading {}: {}", input_path, e);
            process::exit(1);
        }
    };

    match compile(&source) {
        Ok(rust_code) => {
            if let Err(e) = fs::write(&output_path, &rust_code) {
                eprintln!("Error writing {}: {}", output_path, e);
                process::exit(1);
            }
            println!("Compiled {} -> {}", input_path, output_path);
        }
        Err(e) => {
            print_error(&e, &source, input_path);
            process::exit(1);
        }
    }
}
