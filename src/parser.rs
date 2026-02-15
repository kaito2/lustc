use crate::ast::*;
use crate::error::{CompilerError, LustcResult, Span};
use crate::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> LustcResult<Vec<Decl>> {
        let mut decls = Vec::new();
        self.skip_newlines();
        while !self.is_at_end() {
            decls.push(self.parse_decl()?);
            self.skip_newlines();
        }
        Ok(decls)
    }

    // --- Token navigation ---

    fn peek(&self) -> &TokenKind {
        &self.tokens[self.pos].kind
    }

    fn peek_span(&self) -> Span {
        self.tokens[self.pos].span.clone()
    }

    fn advance(&mut self) -> &Token {
        let tok = &self.tokens[self.pos];
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, expected: &TokenKind) -> LustcResult<&Token> {
        if self.peek() == expected {
            Ok(self.advance())
        } else {
            Err(CompilerError::ParseError {
                msg: format!("expected {:?}, found {:?}", expected, self.peek()),
                span: self.peek_span(),
            })
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.peek() == kind
    }

    fn is_at_end(&self) -> bool {
        matches!(self.peek(), TokenKind::Eof)
    }

    fn skip_newlines(&mut self) {
        while matches!(
            self.peek(),
            TokenKind::Newline | TokenKind::Indent | TokenKind::Dedent
        ) {
            self.advance();
        }
    }

    fn skip_newlines_only(&mut self) {
        while matches!(self.peek(), TokenKind::Newline) {
            self.advance();
        }
    }

    // --- Declaration parsers ---

    fn parse_decl(&mut self) -> LustcResult<Decl> {
        match self.peek() {
            TokenKind::Def => self.parse_def(),
            TokenKind::Inductive => self.parse_inductive(),
            TokenKind::HashEval => self.parse_eval(),
            _ => Err(CompilerError::ParseError {
                msg: format!("expected declaration, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }

    fn parse_def(&mut self) -> LustcResult<Decl> {
        self.expect(&TokenKind::Def)?;
        let name = self.parse_ident()?;

        // Parse parameters
        let mut params = Vec::new();
        while self.check(&TokenKind::LParen) {
            self.advance(); // (
            let param_name = self.parse_ident()?;
            self.expect(&TokenKind::Colon)?;
            let param_type = self.parse_type()?;
            self.expect(&TokenKind::RParen)?;
            params.push((param_name, param_type));
        }

        // Parse optional return type
        let return_type = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.skip_newlines();

        // Determine := form vs pattern match form
        if self.check(&TokenKind::ColonEq) {
            self.advance();
            self.skip_newlines();
            let body = self.parse_expr()?;
            Ok(Decl::FunDef {
                name,
                params,
                return_type,
                body,
            })
        } else if self.check(&TokenKind::Newline)
            || self.check(&TokenKind::Indent)
            || self.check(&TokenKind::Pipe)
        {
            // Pattern match form
            self.skip_newlines();
            // Consume indent if present
            if self.check(&TokenKind::Indent) {
                self.advance();
            }
            let cases = self.parse_match_cases()?;
            Ok(Decl::FunDefMatch {
                name,
                params,
                return_type,
                cases,
            })
        } else {
            Err(CompilerError::ParseError {
                msg: format!(
                    "expected ':=' or pattern cases after def, found {:?}",
                    self.peek()
                ),
                span: self.peek_span(),
            })
        }
    }

    fn parse_match_cases(&mut self) -> LustcResult<Vec<(Vec<Pattern>, Expr)>> {
        let mut cases = Vec::new();
        while self.check(&TokenKind::Pipe) {
            self.advance(); // |
            let mut patterns = Vec::new();
            // Parse patterns until =>
            loop {
                if self.check(&TokenKind::FatArrow) {
                    break;
                }
                // Handle comma-separated or space-separated patterns
                patterns.push(self.parse_pattern()?);
                if self.check(&TokenKind::Comma) {
                    self.advance();
                }
            }
            self.expect(&TokenKind::FatArrow)?;
            self.skip_newlines();
            let body = self.parse_expr()?;
            cases.push((patterns, body));
            self.skip_newlines();
        }
        // Consume dedent if present
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }
        Ok(cases)
    }

    fn parse_inductive(&mut self) -> LustcResult<Decl> {
        self.expect(&TokenKind::Inductive)?;
        let name = self.parse_ident()?;
        self.expect(&TokenKind::Where)?;
        self.skip_newlines();

        // Consume indent if present
        if self.check(&TokenKind::Indent) {
            self.advance();
        }

        let mut constructors = Vec::new();
        while self.check(&TokenKind::Pipe) {
            self.advance(); // |
            let ctor_name = self.parse_ident()?;
            let mut fields = Vec::new();

            // Parse optional fields with : Type → Type → ...
            if self.check(&TokenKind::Colon) {
                self.advance();
                // Parse types until we hit something that's not a type continuation
                loop {
                    let ty = self.parse_type_atom()?;
                    // If next is Arrow, this is a parameter type
                    if self.check(&TokenKind::Arrow) {
                        fields.push(ty);
                        self.advance(); // consume ->
                                        // Check if the next type is the result type (same as inductive name)
                        let next_ty = self.parse_type_atom()?;
                        if !self.check(&TokenKind::Arrow) {
                            // This is the return type, don't add to fields
                            let _ = next_ty;
                            break;
                        } else {
                            fields.push(next_ty);
                            self.advance(); // consume ->
                        }
                    } else {
                        // This is the only type or the return type, don't add to fields
                        break;
                    }
                }
            }

            constructors.push(Constructor {
                name: ctor_name,
                fields,
            });
            self.skip_newlines();
        }

        // Consume dedent if present
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(Decl::InductiveDef { name, constructors })
    }

    fn parse_eval(&mut self) -> LustcResult<Decl> {
        self.expect(&TokenKind::HashEval)?;
        self.skip_newlines();
        let expr = self.parse_expr()?;
        Ok(Decl::Eval(expr))
    }

    // --- Type parsers ---

    fn parse_type(&mut self) -> LustcResult<Type> {
        let lhs = self.parse_type_app()?;
        if self.check(&TokenKind::Arrow) {
            self.advance();
            let rhs = self.parse_type()?;
            Ok(Type::Arrow(Box::new(lhs), Box::new(rhs)))
        } else {
            Ok(lhs)
        }
    }

    fn parse_type_app(&mut self) -> LustcResult<Type> {
        let mut ty = self.parse_type_atom()?;
        // Handle type application like `IO Unit`
        while self.is_type_start() {
            let arg = self.parse_type_atom()?;
            ty = Type::App(Box::new(ty), Box::new(arg));
        }
        Ok(ty)
    }

    fn parse_type_atom(&mut self) -> LustcResult<Type> {
        match self.peek() {
            TokenKind::LParen => {
                self.advance();
                if self.check(&TokenKind::RParen) {
                    self.advance();
                    Ok(Type::Unit)
                } else {
                    let ty = self.parse_type()?;
                    self.expect(&TokenKind::RParen)?;
                    Ok(ty)
                }
            }
            TokenKind::Ident(_) => {
                let name = self.parse_ident()?;
                if name == "Unit" {
                    Ok(Type::Unit)
                } else {
                    Ok(Type::Named(name))
                }
            }
            _ => Err(CompilerError::ParseError {
                msg: format!("expected type, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }

    fn is_type_start(&self) -> bool {
        matches!(self.peek(), TokenKind::Ident(_) | TokenKind::LParen)
            && !matches!(
                self.peek(),
                TokenKind::Ident(ref s) if is_expr_keyword(s)
            )
    }

    // --- Expression parsers (precedence climbing) ---

    fn parse_expr(&mut self) -> LustcResult<Expr> {
        self.skip_newlines_only();
        match self.peek() {
            TokenKind::If => self.parse_if(),
            TokenKind::Let => self.parse_let(),
            TokenKind::Match => self.parse_match(),
            TokenKind::Do => self.parse_do(),
            TokenKind::Fun => self.parse_lambda(),
            _ => self.parse_or(),
        }
    }

    fn parse_or(&mut self) -> LustcResult<Expr> {
        let mut lhs = self.parse_and()?;
        while self.check(&TokenKind::Or) {
            self.advance();
            let rhs = self.parse_and()?;
            lhs = Expr::BinOp {
                op: BinOp::Or,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_and(&mut self) -> LustcResult<Expr> {
        let mut lhs = self.parse_comparison()?;
        while self.check(&TokenKind::And) {
            self.advance();
            let rhs = self.parse_comparison()?;
            lhs = Expr::BinOp {
                op: BinOp::And,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_comparison(&mut self) -> LustcResult<Expr> {
        let lhs = self.parse_add()?;
        let op = match self.peek() {
            TokenKind::EqEq => BinOp::Eq,
            TokenKind::Ne => BinOp::Ne,
            TokenKind::Lt => BinOp::Lt,
            TokenKind::Le => BinOp::Le,
            TokenKind::Gt => BinOp::Gt,
            TokenKind::Ge => BinOp::Ge,
            _ => return Ok(lhs),
        };
        self.advance();
        let rhs = self.parse_add()?;
        Ok(Expr::BinOp {
            op,
            lhs: Box::new(lhs),
            rhs: Box::new(rhs),
        })
    }

    fn parse_add(&mut self) -> LustcResult<Expr> {
        let mut lhs = self.parse_mul()?;
        loop {
            let op = match self.peek() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_mul()?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_mul(&mut self) -> LustcResult<Expr> {
        let mut lhs = self.parse_unary()?;
        loop {
            let op = match self.peek() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let rhs = self.parse_unary()?;
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
            };
        }
        Ok(lhs)
    }

    fn parse_unary(&mut self) -> LustcResult<Expr> {
        match self.peek() {
            TokenKind::Not => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                })
            }
            TokenKind::Minus => {
                self.advance();
                let operand = self.parse_unary()?;
                Ok(Expr::UnaryOp {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                })
            }
            _ => self.parse_app(),
        }
    }

    fn parse_app(&mut self) -> LustcResult<Expr> {
        let mut func = self.parse_atom()?;

        // Function application: `f x y` → FunApp(FunApp(f, x), y)
        while self.is_app_arg_start() {
            let arg = self.parse_atom()?;
            func = Expr::FunApp {
                func: Box::new(func),
                arg: Box::new(arg),
            };
        }

        Ok(func)
    }

    fn is_app_arg_start(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::IntLit(_)
                | TokenKind::StringLit(_)
                | TokenKind::True
                | TokenKind::False
                | TokenKind::LParen
        ) || matches!(self.peek(), TokenKind::Ident(ref s) if !is_keyword(s))
    }

    fn parse_atom(&mut self) -> LustcResult<Expr> {
        match self.peek().clone() {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(Expr::IntLit(n))
            }
            TokenKind::StringLit(ref s) => {
                let s = s.clone();
                self.advance();
                Ok(Expr::StringLit(s))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::BoolLit(true))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::BoolLit(false))
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                // Handle qualified names like IO.println or Color.red
                if self.check(&TokenKind::Dot) {
                    let mut qualified = name;
                    while self.check(&TokenKind::Dot) {
                        self.advance();
                        let part = self.parse_ident()?;
                        qualified = format!("{}.{}", qualified, part);
                    }
                    Ok(Expr::Var(qualified))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            TokenKind::LParen => {
                self.advance();
                if self.check(&TokenKind::RParen) {
                    self.advance();
                    return Ok(Expr::Var("()".to_string()));
                }
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(Expr::Paren(Box::new(expr)))
            }
            _ => Err(CompilerError::ParseError {
                msg: format!("expected expression, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }

    fn parse_if(&mut self) -> LustcResult<Expr> {
        self.expect(&TokenKind::If)?;
        self.skip_newlines_only();
        let cond = self.parse_expr()?;
        self.skip_newlines_only();
        self.expect(&TokenKind::Then)?;
        self.skip_newlines_only();
        let then_branch = self.parse_expr()?;
        self.skip_newlines_only();
        self.expect(&TokenKind::Else)?;
        self.skip_newlines_only();
        let else_branch = self.parse_expr()?;
        Ok(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        })
    }

    fn parse_let(&mut self) -> LustcResult<Expr> {
        self.expect(&TokenKind::Let)?;
        let name = self.parse_ident()?;

        let ty = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&TokenKind::ColonEq)?;
        self.skip_newlines_only();
        let value = self.parse_expr()?;
        self.skip_newlines();

        // In a do block, there might not be an explicit `in`
        if self.check(&TokenKind::In) {
            self.advance();
        }
        self.skip_newlines();

        let body = self.parse_expr()?;
        Ok(Expr::Let {
            name,
            ty,
            value: Box::new(value),
            body: Box::new(body),
        })
    }

    fn parse_match(&mut self) -> LustcResult<Expr> {
        self.expect(&TokenKind::Match)?;
        let scrutinee = self.parse_expr()?;
        self.skip_newlines_only();
        self.expect(&TokenKind::With)?;
        self.skip_newlines();

        // Consume indent if present
        if self.check(&TokenKind::Indent) {
            self.advance();
        }

        let mut arms = Vec::new();
        while self.check(&TokenKind::Pipe) {
            self.advance(); // |
            let pattern = self.parse_pattern()?;
            self.expect(&TokenKind::FatArrow)?;
            self.skip_newlines_only();
            let body = self.parse_expr()?;
            arms.push(MatchArm { pattern, body });
            self.skip_newlines();
        }

        // Consume dedent if present
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_do(&mut self) -> LustcResult<Expr> {
        self.expect(&TokenKind::Do)?;
        self.skip_newlines();

        // Consume indent if present
        if self.check(&TokenKind::Indent) {
            self.advance();
        }

        let mut stmts = Vec::new();
        loop {
            self.skip_newlines_only();
            if self.is_at_end()
                || self.check(&TokenKind::Dedent)
                || self.check(&TokenKind::Def)
                || self.check(&TokenKind::Inductive)
                || self.check(&TokenKind::HashEval)
            {
                break;
            }

            if self.check(&TokenKind::Let) {
                // do-let: `let x := expr` (no body/in)
                self.advance(); // let
                let name = self.parse_ident()?;

                // Skip optional type annotation
                if self.check(&TokenKind::Colon) {
                    self.advance();
                    let _ty = self.parse_type()?;
                }

                if self.check(&TokenKind::LeftArrow) {
                    self.advance();
                } else {
                    self.expect(&TokenKind::ColonEq)?;
                }
                self.skip_newlines_only();
                let value = self.parse_expr()?;
                stmts.push(DoStatement::Let(name, value));
            } else {
                let expr = self.parse_expr()?;
                stmts.push(DoStatement::Expr(expr));
            }

            self.skip_newlines();
        }

        // Consume dedent if present
        if self.check(&TokenKind::Dedent) {
            self.advance();
        }

        Ok(Expr::Do(stmts))
    }

    fn parse_lambda(&mut self) -> LustcResult<Expr> {
        self.expect(&TokenKind::Fun)?;
        let mut params = Vec::new();

        while !self.check(&TokenKind::FatArrow) {
            if self.check(&TokenKind::LParen) {
                self.advance();
                let name = self.parse_ident()?;
                let ty = if self.check(&TokenKind::Colon) {
                    self.advance();
                    Some(self.parse_type()?)
                } else {
                    None
                };
                self.expect(&TokenKind::RParen)?;
                params.push((name, ty));
            } else {
                let name = self.parse_ident()?;
                params.push((name, None));
            }
        }
        self.expect(&TokenKind::FatArrow)?;
        let body = self.parse_expr()?;
        Ok(Expr::Lambda {
            params,
            body: Box::new(body),
        })
    }

    // --- Pattern parser ---

    fn parse_pattern(&mut self) -> LustcResult<Pattern> {
        match self.peek().clone() {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(Pattern::IntLit(n))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.advance();

                // Handle qualified constructor: Color.red
                if self.check(&TokenKind::Dot) {
                    let mut qualified = name;
                    while self.check(&TokenKind::Dot) {
                        self.advance();
                        let part = self.parse_ident()?;
                        qualified = format!("{}.{}", qualified, part);
                    }
                    // Parse constructor arguments
                    let mut args = Vec::new();
                    while self.is_pattern_arg_start() {
                        args.push(self.parse_pattern_atom()?);
                    }
                    return Ok(Pattern::Constructor(qualified, args));
                }

                // Check for successor pattern: `n + 1`
                if self.check(&TokenKind::Plus) {
                    self.advance();
                    if let TokenKind::IntLit(k) = self.peek().clone() {
                        self.advance();
                        return Ok(Pattern::Successor(name, k));
                    }
                }

                // Check if it's a constructor (starts with uppercase)
                if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let mut args = Vec::new();
                    while self.is_pattern_arg_start() {
                        args.push(self.parse_pattern_atom()?);
                    }
                    Ok(Pattern::Constructor(name, args))
                } else {
                    Ok(Pattern::Var(name))
                }
            }
            TokenKind::LParen => {
                self.advance();
                let pat = self.parse_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(pat)
            }
            _ => Err(CompilerError::ParseError {
                msg: format!("expected pattern, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }

    fn parse_pattern_atom(&mut self) -> LustcResult<Pattern> {
        match self.peek().clone() {
            TokenKind::IntLit(n) => {
                self.advance();
                Ok(Pattern::IntLit(n))
            }
            TokenKind::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    Ok(Pattern::Constructor(name, vec![]))
                } else {
                    Ok(Pattern::Var(name))
                }
            }
            TokenKind::LParen => {
                self.advance();
                let pat = self.parse_pattern()?;
                self.expect(&TokenKind::RParen)?;
                Ok(pat)
            }
            _ => Err(CompilerError::ParseError {
                msg: format!("expected pattern atom, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }

    fn is_pattern_arg_start(&self) -> bool {
        matches!(
            self.peek(),
            TokenKind::IntLit(_) | TokenKind::Underscore | TokenKind::LParen
        ) || matches!(self.peek(), TokenKind::Ident(ref s) if !is_keyword(s))
    }

    // --- Helpers ---

    fn parse_ident(&mut self) -> LustcResult<String> {
        match self.peek().clone() {
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.advance();
                Ok(name)
            }
            _ => Err(CompilerError::ParseError {
                msg: format!("expected identifier, found {:?}", self.peek()),
                span: self.peek_span(),
            }),
        }
    }
}

fn is_keyword(s: &str) -> bool {
    matches!(
        s,
        "def"
            | "let"
            | "in"
            | "if"
            | "then"
            | "else"
            | "match"
            | "with"
            | "do"
            | "where"
            | "inductive"
            | "fun"
            | "true"
            | "false"
    )
}

fn is_expr_keyword(s: &str) -> bool {
    matches!(
        s,
        "if" | "then" | "else" | "match" | "with" | "do" | "let" | "in" | "fun"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(src: &str) -> Vec<Decl> {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    #[test]
    fn test_simple_def() {
        let decls = parse("def x : Nat := 42");
        assert_eq!(decls.len(), 1);
        match &decls[0] {
            Decl::FunDef { name, body, .. } => {
                assert_eq!(name, "x");
                assert!(matches!(body, Expr::IntLit(42)));
            }
            _ => panic!("expected FunDef"),
        }
    }

    #[test]
    fn test_def_with_params() {
        let decls = parse("def add (x : Nat) (y : Nat) : Nat := x + y");
        assert_eq!(decls.len(), 1);
        match &decls[0] {
            Decl::FunDef { name, params, .. } => {
                assert_eq!(name, "add");
                assert_eq!(params.len(), 2);
            }
            _ => panic!("expected FunDef"),
        }
    }

    #[test]
    fn test_eval() {
        let decls = parse("#eval 1 + 2 * 3");
        assert_eq!(decls.len(), 1);
        assert!(matches!(&decls[0], Decl::Eval(_)));
    }

    #[test]
    fn test_if_expr() {
        let decls = parse("def f (x : Nat) : Nat := if x == 0 then 1 else x");
        match &decls[0] {
            Decl::FunDef { body, .. } => {
                assert!(matches!(body, Expr::If { .. }));
            }
            _ => panic!("expected FunDef"),
        }
    }

    #[test]
    fn test_inductive() {
        let decls = parse("inductive Color where\n  | red\n  | green\n  | blue");
        match &decls[0] {
            Decl::InductiveDef {
                name, constructors, ..
            } => {
                assert_eq!(name, "Color");
                assert_eq!(constructors.len(), 3);
                assert_eq!(constructors[0].name, "red");
            }
            _ => panic!("expected InductiveDef"),
        }
    }

    #[test]
    fn test_function_application() {
        let decls = parse("#eval add 1 2");
        match &decls[0] {
            Decl::Eval(expr) => match expr {
                Expr::FunApp { .. } => {}
                _ => panic!("expected FunApp, got {:?}", expr),
            },
            _ => panic!("expected Eval"),
        }
    }

    #[test]
    fn test_string_literal() {
        let decls = parse(r#"#eval "hello""#);
        match &decls[0] {
            Decl::Eval(Expr::StringLit(s)) => assert_eq!(s, "hello"),
            _ => panic!("expected string literal"),
        }
    }

    #[test]
    fn test_qualified_name() {
        let decls = parse(r#"#eval IO.println "hi""#);
        match &decls[0] {
            Decl::Eval(Expr::FunApp { func, .. }) => match func.as_ref() {
                Expr::Var(name) => assert_eq!(name, "IO.println"),
                _ => panic!("expected qualified var"),
            },
            _ => panic!("expected Eval with FunApp"),
        }
    }
}
