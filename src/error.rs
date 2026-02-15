use std::fmt;

#[derive(Debug, Clone)]
pub struct Span {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
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
    #[allow(dead_code)]
    CodeGenError {
        msg: String,
    },
    IoError(std::io::Error),
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
            CompilerError::CodeGenError { msg } => {
                write!(f, "Code generation error: {}", msg)
            }
            CompilerError::IoError(e) => write!(f, "IO error: {}", e),
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
