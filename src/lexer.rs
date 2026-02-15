use crate::error::{CompilerError, LustcResult, Span};
use crate::token::{Token, TokenKind};

pub struct Lexer {
    source: Vec<char>,
    pos: usize,
    line: usize,
    column: usize,
    indent_stack: Vec<usize>,
    pending_tokens: Vec<Token>,
    at_line_start: bool,
}

impl Lexer {
    pub fn new(source: &str) -> Self {
        Lexer {
            source: source.chars().collect(),
            pos: 0,
            line: 1,
            column: 1,
            indent_stack: vec![0],
            pending_tokens: Vec::new(),
            at_line_start: true,
        }
    }

    pub fn tokenize(&mut self) -> LustcResult<Vec<Token>> {
        let mut tokens = Vec::new();

        loop {
            let tok = self.next_token()?;
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }

        Ok(tokens)
    }

    fn next_token(&mut self) -> LustcResult<Token> {
        // Return pending indent/dedent tokens first
        if let Some(tok) = self.pending_tokens.pop() {
            return Ok(tok);
        }

        // Handle indentation at line start
        if self.at_line_start {
            self.at_line_start = false;
            self.handle_indentation()?;
            if let Some(tok) = self.pending_tokens.pop() {
                return Ok(tok);
            }
        }

        self.skip_spaces();

        if self.is_at_end() {
            // Emit dedents for remaining indent levels
            while self.indent_stack.len() > 1 {
                self.indent_stack.pop();
                self.pending_tokens
                    .push(Token::new(TokenKind::Dedent, self.line, self.column));
            }
            if let Some(tok) = self.pending_tokens.pop() {
                return Ok(tok);
            }
            return Ok(Token::new(TokenKind::Eof, self.line, self.column));
        }

        let ch = self.peek_char();

        // Handle newlines
        if ch == '\n' {
            let tok = Token::new(TokenKind::Newline, self.line, self.column);
            self.advance_char();
            self.at_line_start = true;
            return Ok(tok);
        }

        // Handle comments
        if ch == '-' && self.peek_char_at(1) == Some('-') {
            self.skip_line_comment();
            return self.next_token();
        }
        if ch == '/' && self.peek_char_at(1) == Some('-') {
            self.skip_block_comment()?;
            return self.next_token();
        }

        // Handle #eval
        if ch == '#' {
            return self.lex_hash();
        }

        // String literal
        if ch == '"' {
            return self.lex_string();
        }

        // Number literal
        if ch.is_ascii_digit() {
            return self.lex_number();
        }

        // Identifier or keyword
        if ch.is_alphabetic() || ch == '_' {
            return self.lex_ident();
        }

        // Unicode arrows
        if ch == '→' {
            let tok = Token::new(TokenKind::Arrow, self.line, self.column);
            self.advance_char();
            return Ok(tok);
        }
        if ch == '←' {
            let tok = Token::new(TokenKind::LeftArrow, self.line, self.column);
            self.advance_char();
            return Ok(tok);
        }

        // Operators and delimiters
        self.lex_operator()
    }

    fn handle_indentation(&mut self) -> LustcResult<()> {
        let mut indent = 0;
        while !self.is_at_end() && self.peek_char() == ' ' {
            indent += 1;
            self.advance_char();
        }

        // Skip blank lines and comment-only lines
        if self.is_at_end() || self.peek_char() == '\n' {
            return Ok(());
        }
        // Skip line comments at start of line
        if self.peek_char() == '-' && self.peek_char_at(1) == Some('-') {
            return Ok(());
        }

        let current_indent = *self.indent_stack.last().unwrap();

        if indent > current_indent {
            self.indent_stack.push(indent);
            self.pending_tokens
                .push(Token::new(TokenKind::Indent, self.line, self.column));
        } else {
            while indent < *self.indent_stack.last().unwrap() {
                self.indent_stack.pop();
                self.pending_tokens
                    .push(Token::new(TokenKind::Dedent, self.line, self.column));
            }
        }

        // Reverse so we pop in correct order
        self.pending_tokens.reverse();

        Ok(())
    }

    fn skip_spaces(&mut self) {
        while !self.is_at_end() {
            let ch = self.peek_char();
            if ch == ' ' || ch == '\t' || ch == '\r' {
                self.advance_char();
            } else {
                break;
            }
        }
    }

    fn skip_line_comment(&mut self) {
        while !self.is_at_end() && self.peek_char() != '\n' {
            self.advance_char();
        }
    }

    fn skip_block_comment(&mut self) -> LustcResult<()> {
        let start_span = Span {
            line: self.line,
            column: self.column,
        };
        // consume /-
        self.advance_char();
        self.advance_char();
        let mut depth = 1;
        while !self.is_at_end() && depth > 0 {
            if self.peek_char() == '/' && self.peek_char_at(1) == Some('-') {
                depth += 1;
                self.advance_char();
                self.advance_char();
            } else if self.peek_char() == '-' && self.peek_char_at(1) == Some('/') {
                depth -= 1;
                self.advance_char();
                self.advance_char();
            } else {
                self.advance_char();
            }
        }
        if depth > 0 {
            return Err(CompilerError::LexError {
                msg: "unterminated block comment".to_string(),
                span: start_span,
            });
        }
        Ok(())
    }

    fn lex_hash(&mut self) -> LustcResult<Token> {
        let start_col = self.column;
        let start_line = self.line;
        self.advance_char(); // consume #

        // Try to read "eval"
        let mut word = String::new();
        while !self.is_at_end() && self.peek_char().is_alphabetic() {
            word.push(self.peek_char());
            self.advance_char();
        }
        if word == "eval" {
            Ok(Token::new(TokenKind::HashEval, start_line, start_col))
        } else {
            Err(CompilerError::LexError {
                msg: format!("unexpected directive: #{}", word),
                span: Span {
                    line: start_line,
                    column: start_col,
                },
            })
        }
    }

    fn lex_string(&mut self) -> LustcResult<Token> {
        let start_line = self.line;
        let start_col = self.column;
        self.advance_char(); // consume opening "

        let mut s = String::new();
        while !self.is_at_end() && self.peek_char() != '"' {
            let ch = self.peek_char();
            if ch == '\\' {
                self.advance_char();
                if self.is_at_end() {
                    return Err(CompilerError::LexError {
                        msg: "unterminated string".to_string(),
                        span: Span {
                            line: start_line,
                            column: start_col,
                        },
                    });
                }
                match self.peek_char() {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    '\\' => s.push('\\'),
                    '"' => s.push('"'),
                    other => {
                        s.push('\\');
                        s.push(other);
                    }
                }
            } else {
                s.push(ch);
            }
            self.advance_char();
        }

        if self.is_at_end() {
            return Err(CompilerError::LexError {
                msg: "unterminated string".to_string(),
                span: Span {
                    line: start_line,
                    column: start_col,
                },
            });
        }

        self.advance_char(); // consume closing "
        Ok(Token::new(TokenKind::StringLit(s), start_line, start_col))
    }

    fn lex_number(&mut self) -> LustcResult<Token> {
        let start_col = self.column;
        let start_line = self.line;
        let mut num_str = String::new();
        while !self.is_at_end() && self.peek_char().is_ascii_digit() {
            num_str.push(self.peek_char());
            self.advance_char();
        }
        let value: u64 = num_str.parse().map_err(|_| CompilerError::LexError {
            msg: format!("invalid number: {}", num_str),
            span: Span {
                line: start_line,
                column: start_col,
            },
        })?;
        Ok(Token::new(TokenKind::IntLit(value), start_line, start_col))
    }

    fn lex_ident(&mut self) -> LustcResult<Token> {
        let start_col = self.column;
        let start_line = self.line;
        let mut name = String::new();
        while !self.is_at_end() {
            let ch = self.peek_char();
            if ch.is_alphanumeric() || ch == '_' {
                name.push(ch);
                self.advance_char();
            } else {
                break;
            }
        }

        let kind = match name.as_str() {
            "def" => TokenKind::Def,
            "let" => TokenKind::Let,
            "in" => TokenKind::In,
            "if" => TokenKind::If,
            "then" => TokenKind::Then,
            "else" => TokenKind::Else,
            "match" => TokenKind::Match,
            "with" => TokenKind::With,
            "do" => TokenKind::Do,
            "where" => TokenKind::Where,
            "inductive" => TokenKind::Inductive,
            "fun" => TokenKind::Fun,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Ident(name),
        };

        Ok(Token::new(kind, start_line, start_col))
    }

    fn lex_operator(&mut self) -> LustcResult<Token> {
        let start_col = self.column;
        let start_line = self.line;
        let ch = self.peek_char();
        self.advance_char();

        let kind = match ch {
            '+' => TokenKind::Plus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '(' => TokenKind::LParen,
            ')' => TokenKind::RParen,
            '{' => TokenKind::LBrace,
            '}' => TokenKind::RBrace,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            '|' => {
                if !self.is_at_end() && self.peek_char() == '|' {
                    self.advance_char();
                    TokenKind::Or
                } else {
                    TokenKind::Pipe
                }
            }
            '-' => {
                if !self.is_at_end() && self.peek_char() == '>' {
                    self.advance_char();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '=' => {
                if !self.is_at_end() && self.peek_char() == '>' {
                    self.advance_char();
                    TokenKind::FatArrow
                } else if !self.is_at_end() && self.peek_char() == '=' {
                    self.advance_char();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }
            ':' => {
                if !self.is_at_end() && self.peek_char() == '=' {
                    self.advance_char();
                    TokenKind::ColonEq
                } else {
                    TokenKind::Colon
                }
            }
            '!' => {
                if !self.is_at_end() && self.peek_char() == '=' {
                    self.advance_char();
                    TokenKind::Ne
                } else {
                    TokenKind::Not
                }
            }
            '<' => {
                if !self.is_at_end() && self.peek_char() == '=' {
                    self.advance_char();
                    TokenKind::Le
                } else if !self.is_at_end() && self.peek_char() == '-' {
                    self.advance_char();
                    TokenKind::LeftArrow
                } else {
                    TokenKind::Lt
                }
            }
            '>' => {
                if !self.is_at_end() && self.peek_char() == '=' {
                    self.advance_char();
                    TokenKind::Ge
                } else {
                    TokenKind::Gt
                }
            }
            '&' => {
                if !self.is_at_end() && self.peek_char() == '&' {
                    self.advance_char();
                    TokenKind::And
                } else {
                    return Err(CompilerError::LexError {
                        msg: format!("unexpected character: {}", ch),
                        span: Span {
                            line: start_line,
                            column: start_col,
                        },
                    });
                }
            }
            '_' => TokenKind::Underscore,
            _ => {
                return Err(CompilerError::LexError {
                    msg: format!("unexpected character: {}", ch),
                    span: Span {
                        line: start_line,
                        column: start_col,
                    },
                });
            }
        };

        Ok(Token::new(kind, start_line, start_col))
    }

    fn peek_char(&self) -> char {
        self.source[self.pos]
    }

    fn peek_char_at(&self, offset: usize) -> Option<char> {
        self.source.get(self.pos + offset).copied()
    }

    fn advance_char(&mut self) {
        if !self.is_at_end() {
            if self.source[self.pos] == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }
            self.pos += 1;
        }
    }

    fn is_at_end(&self) -> bool {
        self.pos >= self.source.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token_kinds(src: &str) -> Vec<TokenKind> {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        tokens.into_iter().map(|t| t.kind).collect()
    }

    fn filter_significant(kinds: Vec<TokenKind>) -> Vec<TokenKind> {
        kinds
            .into_iter()
            .filter(|k| {
                !matches!(
                    k,
                    TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
                )
            })
            .collect()
    }

    #[test]
    fn test_simple_def() {
        let kinds = filter_significant(token_kinds("def foo := 42"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Def,
                TokenKind::Ident("foo".to_string()),
                TokenKind::ColonEq,
                TokenKind::IntLit(42),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_operators() {
        let kinds = filter_significant(token_kinds("x + y * z - 1"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("x".to_string()),
                TokenKind::Plus,
                TokenKind::Ident("y".to_string()),
                TokenKind::Star,
                TokenKind::Ident("z".to_string()),
                TokenKind::Minus,
                TokenKind::IntLit(1),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_arrows() {
        let kinds = filter_significant(token_kinds("Nat → Nat"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("Nat".to_string()),
                TokenKind::Arrow,
                TokenKind::Ident("Nat".to_string()),
                TokenKind::Eof,
            ]
        );

        let kinds2 = filter_significant(token_kinds("Nat -> Nat"));
        assert_eq!(
            kinds2,
            vec![
                TokenKind::Ident("Nat".to_string()),
                TokenKind::Arrow,
                TokenKind::Ident("Nat".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_string_literal() {
        let kinds = filter_significant(token_kinds(r#""hello world""#));
        assert_eq!(
            kinds,
            vec![
                TokenKind::StringLit("hello world".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_hash_eval() {
        let kinds = filter_significant(token_kinds("#eval 1 + 2"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::HashEval,
                TokenKind::IntLit(1),
                TokenKind::Plus,
                TokenKind::IntLit(2),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_line_comment() {
        let kinds = filter_significant(token_kinds("x -- comment\ny"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("x".to_string()),
                TokenKind::Ident("y".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_block_comment() {
        let kinds = filter_significant(token_kinds("x /- comment -/ y"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("x".to_string()),
                TokenKind::Ident("y".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_comparison_operators() {
        let kinds = filter_significant(token_kinds("a == b != c <= d >= e"));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Ident("a".to_string()),
                TokenKind::EqEq,
                TokenKind::Ident("b".to_string()),
                TokenKind::Ne,
                TokenKind::Ident("c".to_string()),
                TokenKind::Le,
                TokenKind::Ident("d".to_string()),
                TokenKind::Ge,
                TokenKind::Ident("e".to_string()),
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_indentation() {
        let kinds = token_kinds("do\n  x\n  y\nz");
        // Should have: Do, Newline, Indent, x, Newline, y, Newline, Dedent, z, Eof
        assert!(kinds.contains(&TokenKind::Indent));
        assert!(kinds.contains(&TokenKind::Dedent));
    }

    #[test]
    fn test_keywords() {
        let kinds = filter_significant(token_kinds(
            "def let in if then else match with do where inductive fun true false",
        ));
        assert_eq!(
            kinds,
            vec![
                TokenKind::Def,
                TokenKind::Let,
                TokenKind::In,
                TokenKind::If,
                TokenKind::Then,
                TokenKind::Else,
                TokenKind::Match,
                TokenKind::With,
                TokenKind::Do,
                TokenKind::Where,
                TokenKind::Inductive,
                TokenKind::Fun,
                TokenKind::True,
                TokenKind::False,
                TokenKind::Eof,
            ]
        );
    }
}
