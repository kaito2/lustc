//! Compiler error types and rustc-style diagnostic rendering.
//!
//! `Diagnostic` formats errors with source location, the offending source line,
//! and caret underlines pointing at the error span.

use std::fmt;

#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub column: usize,
    pub end_column: usize,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

pub struct SourceMap {
    lines: Vec<String>,
}

impl SourceMap {
    pub fn new(source: &str) -> Self {
        SourceMap {
            lines: source.lines().map(|l| l.to_string()).collect(),
        }
    }

    pub fn line_text(&self, line: usize) -> Option<&str> {
        if line == 0 || line > self.lines.len() {
            None
        } else {
            Some(&self.lines[line - 1])
        }
    }
}

pub struct Diagnostic<'a> {
    error: &'a CompilerError,
    source_map: &'a SourceMap,
    filename: &'a str,
}

impl<'a> Diagnostic<'a> {
    pub fn new(error: &'a CompilerError, source_map: &'a SourceMap, filename: &'a str) -> Self {
        Diagnostic {
            error,
            source_map,
            filename,
        }
    }
}

impl fmt::Display for Diagnostic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.error {
            CompilerError::LexError { msg, span }
            | CompilerError::ParseError { msg, span }
            | CompilerError::ResolveError { msg, span }
            | CompilerError::TypeError { msg, span } => {
                let label = match self.error {
                    CompilerError::TypeError { .. } => "type error",
                    _ => "error",
                };
                writeln!(f, "{}: {}", label, msg)?;
                writeln!(f, "  --> {}:{}:{}", self.filename, span.line, span.column)?;
                writeln!(f, "   |")?;
                if let Some(line_text) = self.source_map.line_text(span.line) {
                    writeln!(f, "{:>3} | {}", span.line, line_text)?;
                    let underline_len = if span.end_column > span.column {
                        span.end_column - span.column
                    } else {
                        1
                    };
                    let carets: String = "^".repeat(underline_len);
                    writeln!(
                        f,
                        "   | {:>width$}{} {}",
                        "",
                        carets,
                        msg,
                        width = span.column - 1
                    )?;
                }
                Ok(())
            }
            CompilerError::CodeGenError { msg } => {
                writeln!(f, "error: {}", msg)
            }
            CompilerError::ModuleError { msg } => {
                writeln!(f, "module error: {}", msg)
            }
            CompilerError::IoError(e) => {
                writeln!(f, "error: {}", e)
            }
            CompilerError::Multiple(errors) => {
                for (i, err) in errors.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    let diag = Diagnostic::new(err, self.source_map, self.filename);
                    write!(f, "{}", diag)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug)]
#[allow(clippy::enum_variant_names)]
pub enum CompilerError {
    LexError {
        msg: String,
        span: Span,
    },
    ParseError {
        msg: String,
        span: Span,
    },
    ResolveError {
        msg: String,
        span: Span,
    },
    TypeError {
        msg: String,
        span: Span,
    },
    #[allow(dead_code)]
    CodeGenError {
        msg: String,
    },
    ModuleError {
        msg: String,
    },
    IoError(std::io::Error),
    Multiple(Vec<CompilerError>),
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompilerError::LexError { msg, span } => {
                write!(f, "Lex error at {}: {}", span, msg)
            }
            CompilerError::ParseError { msg, span } => {
                write!(f, "Parse error at {}: {}", span, msg)
            }
            CompilerError::ResolveError { msg, span } => {
                write!(f, "Resolve error at {}: {}", span, msg)
            }
            CompilerError::TypeError { msg, span } => {
                write!(f, "Type error at {}: {}", span, msg)
            }
            CompilerError::CodeGenError { msg } => {
                write!(f, "Code generation error: {}", msg)
            }
            CompilerError::ModuleError { msg } => {
                write!(f, "Module error: {}", msg)
            }
            CompilerError::IoError(e) => write!(f, "IO error: {}", e),
            CompilerError::Multiple(errors) => {
                for (i, err) in errors.iter().enumerate() {
                    if i > 0 {
                        writeln!(f)?;
                    }
                    write!(f, "{}", err)?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for CompilerError {}

impl From<std::io::Error> for CompilerError {
    fn from(e: std::io::Error) -> Self {
        CompilerError::IoError(e)
    }
}

pub type LustcResult<T> = Result<T, CompilerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_source_map_line_text() {
        let source = "line one\nline two\nline three";
        let sm = SourceMap::new(source);
        assert_eq!(sm.line_text(1), Some("line one"));
        assert_eq!(sm.line_text(2), Some("line two"));
        assert_eq!(sm.line_text(3), Some("line three"));
        assert_eq!(sm.line_text(0), None);
        assert_eq!(sm.line_text(4), None);
    }

    #[test]
    fn test_source_map_empty_lines() {
        let source = "first\n\nthird";
        let sm = SourceMap::new(source);
        assert_eq!(sm.line_text(1), Some("first"));
        assert_eq!(sm.line_text(2), Some(""));
        assert_eq!(sm.line_text(3), Some("third"));
    }

    #[test]
    fn test_diagnostic_display() {
        let source = "def foo := + 42";
        let sm = SourceMap::new(source);
        let err = CompilerError::ParseError {
            msg: "expected identifier".to_string(),
            span: Span {
                line: 1,
                column: 12,
                end_column: 13,
            },
        };
        let diag = Diagnostic::new(&err, &sm, "input.lean");
        let output = format!("{}", diag);
        assert!(output.contains("error: expected identifier"));
        assert!(output.contains("--> input.lean:1:12"));
        assert!(output.contains("def foo := + 42"));
        assert!(output.contains("^"));
    }
}
