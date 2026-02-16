//! Token types for the Lean4 subset lexer.
//!
//! Includes layout tokens (`Indent`/`Dedent`) produced by the indentation-sensitive
//! lexer to represent block structure without braces.

use crate::error::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Def,
    Let,
    In,
    If,
    Then,
    Else,
    Match,
    With,
    Do,
    Where,
    Inductive,
    Fun,
    Structure,
    Import,
    Open,
    Namespace,
    End,

    // Literals
    IntLit(u64),
    StringLit(String),
    InterpolatedString(String),
    True,
    False,

    // Identifiers
    Ident(String),

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,      // =
    EqEq,    // ==
    Ne,      // !=
    Lt,      // <
    Le,      // <=
    Gt,      // >
    Ge,      // >=
    And,     // &&
    Or,      // ||
    Not,     // !
    ColonEq, // :=

    // Arrows
    Arrow,     // → or ->
    LeftArrow, // ← or <-
    FatArrow,  // =>

    // Product type operator
    Times, // ×

    // Delimiters
    LParen,
    RParen,
    LBrace,
    RBrace,
    Colon,
    Comma,
    Dot,
    Pipe,     // |
    HashEval, // #eval
    Underscore,

    // Layout
    Newline,
    Indent,
    Dedent,

    // Special
    Eof,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, line: usize, column: usize, end_column: usize) -> Self {
        Token {
            kind,
            span: Span {
                line,
                column,
                end_column,
            },
        }
    }
}
