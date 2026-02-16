use crate::ast::*;
use crate::error::CompilerError;

pub struct Resolver {
    scopes: Vec<Vec<String>>,
    errors: Vec<CompilerError>,
}

impl Resolver {
    pub fn new() -> Self {
        let builtins = vec![
            "IO.println".to_string(),
            "toString".to_string(),
            "()".to_string(),
            "List.nil".to_string(),
            "List.cons".to_string(),
            "Option.none".to_string(),
            "Option.some".to_string(),
        ];
        Resolver {
            scopes: vec![builtins],
            errors: Vec::new(),
        }
    }

    pub fn resolve(mut self, decls: &[Decl]) -> Result<(), Vec<CompilerError>> {
        // First pass: collect all top-level names
        let mut top_level = Vec::new();
        self.collect_names(decls, "", &mut top_level);
        self.scopes.push(top_level);

        // Process open declarations: add unqualified aliases for opened namespace members
        self.process_opens(decls);

        // Second pass: resolve names in all declarations
        for decl in decls {
            self.resolve_decl(decl);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn collect_names(&self, decls: &[Decl], prefix: &str, names: &mut Vec<String>) {
        for decl in decls {
            match decl {
                Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } => {
                    names.push(name.clone());
                    if !prefix.is_empty() {
                        names.push(format!("{}.{}", prefix, name));
                    }
                }
                Decl::InductiveDef { name, constructors } => {
                    names.push(name.clone());
                    if !prefix.is_empty() {
                        names.push(format!("{}.{}", prefix, name));
                    }
                    for ctor in constructors {
                        names.push(ctor.name.clone());
                        names.push(format!("{}.{}", name, ctor.name));
                        if !prefix.is_empty() {
                            names.push(format!("{}.{}.{}", prefix, name, ctor.name));
                        }
                    }
                }
                Decl::StructDef { name, fields } => {
                    names.push(name.clone());
                    if !prefix.is_empty() {
                        names.push(format!("{}.{}", prefix, name));
                    }
                    names.push(format!("{}.mk", name));
                    if !prefix.is_empty() {
                        names.push(format!("{}.{}.mk", prefix, name));
                    }
                    for (fname, _) in fields {
                        names.push(format!("{}.{}", name, fname));
                        if !prefix.is_empty() {
                            names.push(format!("{}.{}.{}", prefix, name, fname));
                        }
                    }
                }
                Decl::Namespace { name, decls: inner } => {
                    names.push(name.clone());
                    if !prefix.is_empty() {
                        names.push(format!("{}.{}", prefix, name));
                    }
                    let new_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}.{}", prefix, name)
                    };
                    // Also register inner names qualified with just this namespace name
                    // e.g., import Utils.Math → Math.add should work
                    self.collect_names(inner, name, names);
                    if !prefix.is_empty() {
                        self.collect_names(inner, &new_prefix, names);
                    }
                }
                Decl::Import { .. } | Decl::Open { .. } | Decl::Eval(_) => {}
            }
        }
    }

    fn process_opens(&mut self, decls: &[Decl]) {
        for decl in decls {
            if let Decl::Open { path } = decl {
                let ns_prefix = path.segments.join(".");
                // Find all qualified names with this prefix and add unqualified versions
                let scope = self.scopes.last().cloned().unwrap_or_default();
                let mut to_add = Vec::new();
                let prefix_dot = format!("{}.", ns_prefix);
                for name in &scope {
                    if let Some(rest) = name.strip_prefix(&prefix_dot) {
                        if !rest.contains('.') {
                            to_add.push(rest.to_string());
                        }
                    }
                }
                if let Some(scope) = self.scopes.last_mut() {
                    scope.extend(to_add);
                }
            }
        }
    }

    fn resolve_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::FunDef {
                params, body, ..
            } => {
                self.push_scope();
                for (name, _) in params {
                    self.define(name.clone());
                }
                self.resolve_expr(body);
                self.pop_scope();
            }
            Decl::FunDefMatch {
                params, cases, ..
            } => {
                self.push_scope();
                for (name, _) in params {
                    self.define(name.clone());
                }
                for (patterns, body) in cases {
                    self.push_scope();
                    for pat in patterns {
                        self.define_pattern_vars(pat);
                    }
                    self.resolve_expr(body);
                    self.pop_scope();
                }
                self.pop_scope();
            }
            Decl::InductiveDef { .. } => {}
            Decl::StructDef { .. } => {}
            Decl::Eval(expr) => {
                self.resolve_expr(expr);
            }
            Decl::Import { .. } | Decl::Open { .. } => {}
            Decl::Namespace { decls, .. } => {
                for d in decls {
                    self.resolve_decl(d);
                }
            }
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Var(name, span) => {
                if !self.is_defined(name) {
                    // Handle tuple field access: p.1, p.2 → check base 'p'
                    let is_tuple_access = if let Some(dot_pos) = name.rfind('.') {
                        let field = &name[dot_pos + 1..];
                        let base = &name[..dot_pos];
                        field.parse::<usize>().is_ok() && self.is_defined(base)
                    } else {
                        false
                    };
                    if !is_tuple_access {
                        self.errors.push(CompilerError::ResolveError {
                            msg: format!("undefined variable `{}`", name),
                            span: span.clone(),
                        });
                    }
                }
            }
            Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) => {}
            Expr::BinOp { lhs, rhs, .. } => {
                self.resolve_expr(lhs);
                self.resolve_expr(rhs);
            }
            Expr::UnaryOp { operand, .. } => {
                self.resolve_expr(operand);
            }
            Expr::FunApp { func, arg } => {
                self.resolve_expr(func);
                self.resolve_expr(arg);
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.resolve_expr(cond);
                self.resolve_expr(then_branch);
                self.resolve_expr(else_branch);
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.resolve_expr(value);
                self.push_scope();
                self.define(name.clone());
                self.resolve_expr(body);
                self.pop_scope();
            }
            Expr::Match { scrutinee, arms } => {
                self.resolve_expr(scrutinee);
                for arm in arms {
                    self.push_scope();
                    self.define_pattern_vars(&arm.pattern);
                    self.resolve_expr(&arm.body);
                    self.pop_scope();
                }
            }
            Expr::Do(stmts) => {
                self.push_scope();
                for stmt in stmts {
                    match stmt {
                        DoStatement::Expr(e) => self.resolve_expr(e),
                        DoStatement::Let(name, value) => {
                            self.resolve_expr(value);
                            self.define(name.clone());
                        }
                    }
                }
                self.pop_scope();
            }
            Expr::Lambda { params, body } => {
                self.push_scope();
                for (name, _) in params {
                    self.define(name.clone());
                }
                self.resolve_expr(body);
                self.pop_scope();
            }
            Expr::Paren(inner) => {
                self.resolve_expr(inner);
            }
            Expr::Tuple(elems) => {
                for e in elems {
                    self.resolve_expr(e);
                }
            }
            Expr::StringInterpolation(parts) => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.resolve_expr(e);
                    }
                }
            }
        }
    }

    fn define_pattern_vars(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Var(name) => self.define(name.clone()),
            Pattern::Constructor(_, args) => {
                for arg in args {
                    self.define_pattern_vars(arg);
                }
            }
            Pattern::Successor(name, _) => self.define(name.clone()),
            Pattern::Tuple(pats) => {
                for p in pats {
                    self.define_pattern_vars(p);
                }
            }
            Pattern::IntLit(_) | Pattern::Wildcard => {}
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: String) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(name);
        }
    }

    fn is_defined(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.iter().any(|n| n == name) {
                return true;
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn parse(src: &str) -> Vec<Decl> {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        parser.parse_program().unwrap()
    }

    fn resolve(src: &str) -> Result<(), Vec<CompilerError>> {
        let decls = parse(src);
        let resolver = Resolver::new();
        resolver.resolve(&decls)
    }

    #[test]
    fn test_defined_variable_ok() {
        assert!(resolve("def f (x : Nat) : Nat := x + 1").is_ok());
    }

    #[test]
    fn test_undefined_variable_error() {
        let result = resolve("def f (x : Nat) : Nat := y + 1");
        match result {
            Err(errors) => {
                assert_eq!(errors.len(), 1);
                match &errors[0] {
                    CompilerError::ResolveError { msg, .. } => {
                        assert!(msg.contains("undefined variable `y`"));
                    }
                    other => panic!("expected ResolveError, got {:?}", other),
                }
            }
            Ok(_) => panic!("expected error"),
        }
    }

    #[test]
    fn test_let_binding_scope() {
        assert!(resolve("#eval let x := 10 in x + 1").is_ok());
    }

    #[test]
    fn test_lambda_scope() {
        assert!(resolve("#eval (fun n => n + 1) 5").is_ok());
    }

    #[test]
    fn test_match_pattern_scope() {
        let src = r#"inductive Color where
  | red
  | green
  | blue

def f (c : Color) : Nat :=
  match c with
  | Color.red => 1
  | Color.green => 2
  | Color.blue => 3"#;
        assert!(resolve(src).is_ok());
    }

    #[test]
    fn test_recursive_function() {
        let src = "def factorial (n : Nat) : Nat := if n == 0 then 1 else n * factorial (n - 1)";
        assert!(resolve(src).is_ok());
    }

    #[test]
    fn test_builtins() {
        assert!(resolve(r#"def main : IO Unit := do
  IO.println (toString 42)"#).is_ok());
    }

    #[test]
    fn test_do_let_scope() {
        assert!(resolve(r#"def main : IO Unit := do
  let x := 10
  IO.println (toString x)"#).is_ok());
    }

    #[test]
    fn test_pattern_match_function_def() {
        let src = "def factorial : Nat → Nat\n  | 0 => 1\n  | n + 1 => (n + 1) * factorial n";
        assert!(resolve(src).is_ok());
    }

    #[test]
    fn test_struct_names_resolved() {
        let src = r#"structure Point where
  x : Nat
  y : Nat

def origin : Point := Point.mk 0 0"#;
        assert!(resolve(src).is_ok());
    }

    #[test]
    fn test_list_option_builtins() {
        assert!(resolve("#eval List.nil").is_ok());
        assert!(resolve("#eval List.cons 1 List.nil").is_ok());
        assert!(resolve("#eval Option.none").is_ok());
        assert!(resolve("#eval Option.some 42").is_ok());
    }
}
