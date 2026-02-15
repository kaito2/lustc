use crate::ast::*;
use crate::error::{CompilerError, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Ty {
    Nat,
    Int,
    Bool,
    String,
    Unit,
    Arrow(Box<Ty>, Box<Ty>),
    Tuple(Vec<Ty>),
    List(Box<Ty>),
    Option(Box<Ty>),
    Named(std::string::String),
    /// Unknown type — used when inference cannot determine a type
    Unknown,
}

impl std::fmt::Display for Ty {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ty::Nat => write!(f, "Nat"),
            Ty::Int => write!(f, "Int"),
            Ty::Bool => write!(f, "Bool"),
            Ty::String => write!(f, "String"),
            Ty::Unit => write!(f, "Unit"),
            Ty::Arrow(from, to) => write!(f, "{} → {}", from, to),
            Ty::Tuple(tys) => {
                let parts: Vec<_> = tys.iter().map(|t| t.to_string()).collect();
                write!(f, "({})", parts.join(", "))
            }
            Ty::List(inner) => write!(f, "List {}", inner),
            Ty::Option(inner) => write!(f, "Option {}", inner),
            Ty::Named(name) => write!(f, "{}", name),
            Ty::Unknown => write!(f, "?"),
        }
    }
}

pub struct TypeChecker {
    scopes: Vec<Vec<(std::string::String, Ty)>>,
    errors: Vec<CompilerError>,
    /// Type definitions: inductive type name → constructor signatures
    type_defs: Vec<(std::string::String, Vec<(std::string::String, Ty)>)>,
    /// Struct definitions: struct name → field types
    struct_defs: Vec<(std::string::String, Vec<(std::string::String, Ty)>)>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let builtins = vec![
            (
                "IO.println".to_string(),
                Ty::Arrow(Box::new(Ty::String), Box::new(Ty::Unit)),
            ),
            (
                "toString".to_string(),
                Ty::Arrow(Box::new(Ty::Unknown), Box::new(Ty::String)),
            ),
            ("()".to_string(), Ty::Unit),
            ("List.nil".to_string(), Ty::List(Box::new(Ty::Unknown))),
            (
                "List.cons".to_string(),
                Ty::Arrow(
                    Box::new(Ty::Unknown),
                    Box::new(Ty::Arrow(
                        Box::new(Ty::List(Box::new(Ty::Unknown))),
                        Box::new(Ty::List(Box::new(Ty::Unknown))),
                    )),
                ),
            ),
            (
                "Option.none".to_string(),
                Ty::Option(Box::new(Ty::Unknown)),
            ),
            (
                "Option.some".to_string(),
                Ty::Arrow(
                    Box::new(Ty::Unknown),
                    Box::new(Ty::Option(Box::new(Ty::Unknown))),
                ),
            ),
        ];
        TypeChecker {
            scopes: vec![builtins],
            errors: Vec::new(),
            type_defs: Vec::new(),
            struct_defs: Vec::new(),
        }
    }

    pub fn check(mut self, decls: &[Decl]) -> Result<(), Vec<CompilerError>> {
        // First pass: collect all type definitions and function signatures
        let mut top_level = Vec::new();
        for decl in decls {
            match decl {
                Decl::FunDef {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    let ty = self.build_fun_type(params, return_type);
                    top_level.push((name.clone(), ty));
                }
                Decl::FunDefMatch {
                    name,
                    params,
                    return_type,
                    ..
                } => {
                    let ty = self.build_fun_type(params, return_type);
                    top_level.push((name.clone(), ty));
                }
                Decl::InductiveDef { name, constructors } => {
                    let result_ty = Ty::Named(name.clone());
                    let mut ctor_sigs = Vec::new();
                    for ctor in constructors {
                        let ctor_ty = if ctor.fields.is_empty() {
                            result_ty.clone()
                        } else {
                            let mut ty = result_ty.clone();
                            for field in ctor.fields.iter().rev() {
                                ty = Ty::Arrow(Box::new(self.ast_type_to_ty(field)), Box::new(ty));
                            }
                            ty
                        };
                        let qualified = format!("{}.{}", name, ctor.name);
                        top_level.push((ctor.name.clone(), ctor_ty.clone()));
                        top_level.push((qualified.clone(), ctor_ty.clone()));
                        ctor_sigs.push((ctor.name.clone(), ctor_ty));
                    }
                    top_level.push((name.clone(), result_ty));
                    self.type_defs.push((name.clone(), ctor_sigs));
                }
                Decl::StructDef { name, fields } => {
                    let result_ty = Ty::Named(name.clone());
                    // Register Name.mk constructor
                    let mut mk_ty = result_ty.clone();
                    for (_, ftype) in fields.iter().rev() {
                        mk_ty =
                            Ty::Arrow(Box::new(self.ast_type_to_ty(ftype)), Box::new(mk_ty));
                    }
                    top_level.push((format!("{}.mk", name), mk_ty));

                    // Register Name.field accessors
                    let mut field_sigs = Vec::new();
                    for (fname, ftype) in fields {
                        let accessor_ty = Ty::Arrow(
                            Box::new(result_ty.clone()),
                            Box::new(self.ast_type_to_ty(ftype)),
                        );
                        top_level.push((format!("{}.{}", name, fname), accessor_ty.clone()));
                        field_sigs.push((fname.clone(), self.ast_type_to_ty(ftype)));
                    }
                    top_level.push((name.clone(), result_ty));
                    self.struct_defs.push((name.clone(), field_sigs));
                }
                Decl::Eval(_) => {}
            }
        }
        self.scopes.push(top_level);

        // Second pass: type-check all declarations
        for decl in decls {
            self.check_decl(decl);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn build_fun_type(&self, params: &[(std::string::String, Type)], return_type: &Option<Type>) -> Ty {
        let ret = return_type
            .as_ref()
            .map(|t| self.ast_type_to_ty(t))
            .unwrap_or(Ty::Unknown);
        if params.is_empty() {
            ret
        } else {
            let mut ty = ret;
            for (_, ptype) in params.iter().rev() {
                ty = Ty::Arrow(Box::new(self.ast_type_to_ty(ptype)), Box::new(ty));
            }
            ty
        }
    }

    fn ast_type_to_ty(&self, ty: &Type) -> Ty {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Nat" => Ty::Nat,
                "Int" => Ty::Int,
                "Bool" => Ty::Bool,
                "String" => Ty::String,
                other => Ty::Named(other.to_string()),
            },
            Type::Arrow(from, to) => {
                Ty::Arrow(Box::new(self.ast_type_to_ty(from)), Box::new(self.ast_type_to_ty(to)))
            }
            Type::App(base, arg) => {
                if let Type::Named(name) = base.as_ref() {
                    match name.as_str() {
                        "IO" => Ty::Unit, // IO Unit → Unit for simplicity
                        "List" => Ty::List(Box::new(self.ast_type_to_ty(arg))),
                        "Option" => Ty::Option(Box::new(self.ast_type_to_ty(arg))),
                        _ => Ty::Named(name.clone()),
                    }
                } else {
                    Ty::Unknown
                }
            }
            Type::Tuple(types) => {
                Ty::Tuple(types.iter().map(|t| self.ast_type_to_ty(t)).collect())
            }
            Type::Unit => Ty::Unit,
        }
    }

    fn check_decl(&mut self, decl: &Decl) {
        match decl {
            Decl::FunDef {
                params,
                return_type,
                body,
                ..
            } => {
                self.push_scope();
                for (name, ptype) in params {
                    self.define(name.clone(), self.ast_type_to_ty(ptype));
                }
                let body_ty = self.infer_expr(body);
                if let Some(ret) = return_type {
                    let expected = self.ast_type_to_ty(ret);
                    self.check_compatible(&expected, &body_ty, &self.expr_span(body));
                }
                self.pop_scope();
            }
            Decl::FunDefMatch {
                params,
                return_type,
                cases,
                ..
            } => {
                self.push_scope();
                for (name, ptype) in params {
                    self.define(name.clone(), self.ast_type_to_ty(ptype));
                }
                // For pattern match defs, decompose arrow type to get the actual return type.
                // e.g. `def factorial : Nat → Nat` has return_type = Arrow(Nat, Nat),
                // where the first Nat is the match param type and the second is the body type.
                let (match_param_types, actual_ret) = if let Some(ref rt) = return_type {
                    let full_ty = self.ast_type_to_ty(rt);
                    self.decompose_arrow(full_ty)
                } else {
                    (vec![], Ty::Unknown)
                };
                for (patterns, body) in cases {
                    self.push_scope();
                    for (i, pat) in patterns.iter().enumerate() {
                        let expected_pat_ty = match_param_types.get(i).cloned().unwrap_or(Ty::Unknown);
                        self.define_pattern_types(pat, &expected_pat_ty);
                    }
                    let body_ty = self.infer_expr(body);
                    self.check_compatible(&actual_ret, &body_ty, &self.expr_span(body));
                    self.pop_scope();
                }
                self.pop_scope();
            }
            Decl::InductiveDef { .. } | Decl::StructDef { .. } => {}
            Decl::Eval(expr) => {
                self.infer_expr(expr);
            }
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> Ty {
        match expr {
            Expr::IntLit(_) => Ty::Nat,
            Expr::StringLit(_) => Ty::String,
            Expr::BoolLit(_) => Ty::Bool,
            Expr::Var(name, _span) => self.lookup(name).unwrap_or(Ty::Unknown),
            Expr::BinOp { op, lhs, rhs } => {
                let lhs_ty = self.infer_expr(lhs);
                let rhs_ty = self.infer_expr(rhs);
                match op {
                    BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => {
                        // Arithmetic ops: both sides should be numeric
                        self.check_numeric(&lhs_ty, &self.expr_span(lhs));
                        self.check_numeric(&rhs_ty, &self.expr_span(rhs));
                        // Result type follows lhs
                        if matches!(lhs_ty, Ty::Unknown) {
                            rhs_ty
                        } else {
                            lhs_ty
                        }
                    }
                    BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                        Ty::Bool
                    }
                    BinOp::And | BinOp::Or => {
                        self.check_compatible(&Ty::Bool, &lhs_ty, &self.expr_span(lhs));
                        self.check_compatible(&Ty::Bool, &rhs_ty, &self.expr_span(rhs));
                        Ty::Bool
                    }
                }
            }
            Expr::UnaryOp { op, operand } => {
                let operand_ty = self.infer_expr(operand);
                match op {
                    UnaryOp::Not => {
                        self.check_compatible(&Ty::Bool, &operand_ty, &self.expr_span(operand));
                        Ty::Bool
                    }
                    UnaryOp::Neg => {
                        self.check_numeric(&operand_ty, &self.expr_span(operand));
                        operand_ty
                    }
                }
            }
            Expr::FunApp { func, arg } => {
                let func_ty = self.infer_expr(func);
                let _arg_ty = self.infer_expr(arg);
                match func_ty {
                    Ty::Arrow(_, ret) => *ret,
                    Ty::Unknown => Ty::Unknown,
                    _ => {
                        // Don't error on Named types — they might be constructors
                        // that we don't have full type info for
                        Ty::Unknown
                    }
                }
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let cond_ty = self.infer_expr(cond);
                self.check_compatible(&Ty::Bool, &cond_ty, &self.expr_span(cond));
                let then_ty = self.infer_expr(then_branch);
                let else_ty = self.infer_expr(else_branch);
                self.check_compatible(&then_ty, &else_ty, &self.expr_span(else_branch));
                then_ty
            }
            Expr::Let {
                name, ty, value, body, ..
            } => {
                let value_ty = self.infer_expr(value);
                let declared_ty = if let Some(t) = ty {
                    let dt = self.ast_type_to_ty(t);
                    self.check_compatible(&dt, &value_ty, &self.expr_span(value));
                    dt
                } else {
                    value_ty
                };
                self.push_scope();
                self.define(name.clone(), declared_ty);
                let body_ty = self.infer_expr(body);
                self.pop_scope();
                body_ty
            }
            Expr::Match { scrutinee, arms } => {
                let scrutinee_ty = self.infer_expr(scrutinee);
                let mut result_ty = Ty::Unknown;
                for arm in arms {
                    self.push_scope();
                    self.define_pattern_types(&arm.pattern, &scrutinee_ty);
                    let arm_ty = self.infer_expr(&arm.body);
                    if matches!(result_ty, Ty::Unknown) {
                        result_ty = arm_ty.clone();
                    } else {
                        self.check_compatible(&result_ty, &arm_ty, &self.expr_span(&arm.body));
                    }
                    self.pop_scope();
                }
                result_ty
            }
            Expr::Do(stmts) => {
                self.push_scope();
                let mut last_ty = Ty::Unit;
                for stmt in stmts {
                    match stmt {
                        DoStatement::Expr(e) => {
                            last_ty = self.infer_expr(e);
                        }
                        DoStatement::Let(name, value) => {
                            let value_ty = self.infer_expr(value);
                            self.define(name.clone(), value_ty);
                            last_ty = Ty::Unit;
                        }
                    }
                }
                self.pop_scope();
                last_ty
            }
            Expr::Lambda { params, body } => {
                self.push_scope();
                let mut param_types = Vec::new();
                for (name, ty_ann) in params {
                    let ty = ty_ann
                        .as_ref()
                        .map(|t| self.ast_type_to_ty(t))
                        .unwrap_or(Ty::Unknown);
                    self.define(name.clone(), ty.clone());
                    param_types.push(ty);
                }
                let body_ty = self.infer_expr(body);
                self.pop_scope();
                // Build arrow type
                let mut ty = body_ty;
                for pt in param_types.into_iter().rev() {
                    ty = Ty::Arrow(Box::new(pt), Box::new(ty));
                }
                ty
            }
            Expr::Paren(inner) => self.infer_expr(inner),
            Expr::Tuple(elems) => {
                let tys: Vec<Ty> = elems.iter().map(|e| self.infer_expr(e)).collect();
                Ty::Tuple(tys)
            }
            Expr::StringInterpolation(parts) => {
                for part in parts {
                    if let StringInterpPart::Expr(e) = part {
                        self.infer_expr(e);
                    }
                }
                Ty::String
            }
        }
    }

    fn define_pattern_types(&mut self, pattern: &Pattern, expected: &Ty) {
        match pattern {
            Pattern::Var(name) => self.define(name.clone(), expected.clone()),
            Pattern::Constructor(_, args) => {
                for arg in args {
                    self.define_pattern_types(arg, &Ty::Unknown);
                }
            }
            Pattern::Successor(name, _) => self.define(name.clone(), Ty::Nat),
            Pattern::Tuple(pats) => {
                if let Ty::Tuple(tys) = expected {
                    for (i, p) in pats.iter().enumerate() {
                        let ty = tys.get(i).cloned().unwrap_or(Ty::Unknown);
                        self.define_pattern_types(p, &ty);
                    }
                } else {
                    for p in pats {
                        self.define_pattern_types(p, &Ty::Unknown);
                    }
                }
            }
            Pattern::IntLit(_) | Pattern::Wildcard => {}
        }
    }

    /// Decompose an arrow type into parameter types and return type.
    /// e.g. Arrow(A, Arrow(B, C)) → ([A, B], C)
    fn decompose_arrow(&self, ty: Ty) -> (Vec<Ty>, Ty) {
        let mut params = Vec::new();
        let mut current = ty;
        while let Ty::Arrow(from, to) = current {
            params.push(*from);
            current = *to;
        }
        (params, current)
    }

    fn check_compatible(&mut self, expected: &Ty, actual: &Ty, span: &Span) {
        // Unknown is always compatible (we can't check what we don't know)
        if matches!(expected, Ty::Unknown) || matches!(actual, Ty::Unknown) {
            return;
        }
        if !self.types_match(expected, actual) {
            self.errors.push(CompilerError::TypeError {
                msg: format!("type mismatch: expected `{}`, found `{}`", expected, actual),
                span: span.clone(),
            });
        }
    }

    fn check_numeric(&mut self, ty: &Ty, span: &Span) {
        if matches!(ty, Ty::Unknown | Ty::Nat | Ty::Int) {
            return;
        }
        self.errors.push(CompilerError::TypeError {
            msg: format!("expected numeric type, found `{}`", ty),
            span: span.clone(),
        });
    }

    fn types_match(&self, a: &Ty, b: &Ty) -> bool {
        match (a, b) {
            (Ty::Unknown, _) | (_, Ty::Unknown) => true,
            (Ty::Nat, Ty::Nat) => true,
            (Ty::Int, Ty::Int) => true,
            (Ty::Bool, Ty::Bool) => true,
            (Ty::String, Ty::String) => true,
            (Ty::Unit, Ty::Unit) => true,
            (Ty::Arrow(a1, a2), Ty::Arrow(b1, b2)) => {
                self.types_match(a1, b1) && self.types_match(a2, b2)
            }
            (Ty::Tuple(a_tys), Ty::Tuple(b_tys)) => {
                a_tys.len() == b_tys.len()
                    && a_tys.iter().zip(b_tys.iter()).all(|(a, b)| self.types_match(a, b))
            }
            (Ty::List(a), Ty::List(b)) => self.types_match(a, b),
            (Ty::Option(a), Ty::Option(b)) => self.types_match(a, b),
            (Ty::Named(a), Ty::Named(b)) => a == b,
            // Nat and Int are not interchangeable
            _ => false,
        }
    }

    fn expr_span(&self, expr: &Expr) -> Span {
        match expr {
            Expr::Var(_, span) => span.clone(),
            _ => Span {
                line: 0,
                column: 0,
                end_column: 0,
            },
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: std::string::String, ty: Ty) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name, ty));
        }
    }

    fn lookup(&self, name: &str) -> Option<Ty> {
        // Handle tuple field access: p.1 → lookup p, return Unknown
        if let Some(dot_pos) = name.rfind('.') {
            let field = &name[dot_pos + 1..];
            let base = &name[..dot_pos];
            if field.parse::<usize>().is_ok() {
                if let Some(Ty::Tuple(tys)) = self.lookup(base).as_ref() {
                    let idx: usize = field.parse().unwrap();
                    if idx >= 1 && idx <= tys.len() {
                        return Some(tys[idx - 1].clone());
                    }
                }
                return Some(Ty::Unknown);
            }
        }
        for scope in self.scopes.iter().rev() {
            for (n, ty) in scope.iter().rev() {
                if n == name {
                    return Some(ty.clone());
                }
            }
        }
        None
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

    fn typecheck(src: &str) -> Result<(), Vec<CompilerError>> {
        let decls = parse(src);
        let checker = TypeChecker::new();
        checker.check(&decls)
    }

    #[test]
    fn test_well_typed_arithmetic() {
        assert!(typecheck("def add (x : Nat) (y : Nat) : Nat := x + y").is_ok());
    }

    #[test]
    fn test_bool_condition() {
        assert!(typecheck("def f (x : Nat) : Nat := if x == 0 then 1 else x").is_ok());
    }

    #[test]
    fn test_type_mismatch_if_branches() {
        let result = typecheck("def f (x : Nat) : Nat := if x == 0 then 1 else true");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, CompilerError::TypeError { msg, .. } if msg.contains("type mismatch"))));
    }

    #[test]
    fn test_arithmetic_on_bool() {
        let result = typecheck("def f (x : Bool) : Nat := x + 1");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, CompilerError::TypeError { msg, .. } if msg.contains("numeric"))));
    }

    #[test]
    fn test_and_on_non_bool() {
        let result = typecheck("def f (x : Nat) (y : Nat) : Bool := x && y");
        assert!(result.is_err());
    }

    #[test]
    fn test_let_binding_type() {
        assert!(typecheck("#eval let x := 10 in x + 1").is_ok());
    }

    #[test]
    fn test_lambda_infer() {
        assert!(typecheck("#eval (fun n => n + 1) 5").is_ok());
    }

    #[test]
    fn test_string_interpolation_returns_string() {
        assert!(typecheck(r#"def main : IO Unit := do
  let name := "world"
  IO.println s!"Hello, {name}!""#).is_ok());
    }

    #[test]
    fn test_tuple_type() {
        assert!(typecheck("def fst (p : Nat × Nat) : Nat := p.1").is_ok());
    }

    #[test]
    fn test_match_arms_type_consistency() {
        let result = typecheck(r#"inductive Color where
  | red
  | green
  | blue

def f (c : Color) : Nat :=
  match c with
  | Color.red => 1
  | Color.green => "hello"
  | Color.blue => 3"#);
        assert!(result.is_err());
    }

    #[test]
    fn test_return_type_mismatch() {
        let result = typecheck("def f (x : Nat) : String := x + 1");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors.iter().any(|e| matches!(e, CompilerError::TypeError { msg, .. } if msg.contains("type mismatch"))));
    }

    #[test]
    fn test_recursive_function() {
        assert!(typecheck("def factorial (n : Nat) : Nat := if n == 0 then 1 else n * factorial (n - 1)").is_ok());
    }

    #[test]
    fn test_struct_type_check() {
        assert!(typecheck(r#"structure Point where
  x : Nat
  y : Nat

def origin : Point := Point.mk 0 0"#).is_ok());
    }

    #[test]
    fn test_option_type_check() {
        assert!(typecheck(r#"def showOpt (o : Option Nat) : String :=
  match o with
  | Option.none => "nothing"
  | Option.some x => s!"got {x}""#).is_ok());
    }
}
