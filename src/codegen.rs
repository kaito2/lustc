use crate::ast::*;
use crate::error::LustcResult;

pub struct CodeGen {
    output: String,
    indent: usize,
    inductive_names: Vec<String>,
}

impl CodeGen {
    pub fn new() -> Self {
        CodeGen {
            output: String::new(),
            indent: 0,
            inductive_names: Vec::new(),
        }
    }

    pub fn generate(&mut self, decls: &[Decl]) -> LustcResult<String> {
        // First pass: collect inductive type names
        for decl in decls {
            if let Decl::InductiveDef { name, .. } = decl {
                self.inductive_names.push(name.clone());
            }
        }

        // Check if there are #eval declarations that need a main function
        let has_main_def = decls.iter().any(|d| matches!(d, Decl::FunDef { name, .. } | Decl::FunDefMatch { name, .. } if name == "main"));
        let eval_exprs: Vec<&Expr> = decls
            .iter()
            .filter_map(|d| match d {
                Decl::Eval(e) => Some(e),
                _ => None,
            })
            .collect();

        for decl in decls {
            match decl {
                Decl::Eval(_) => {} // handled below
                _ => {
                    self.gen_decl(decl)?;
                    self.output.push('\n');
                }
            }
        }

        // Generate main for #eval if no main exists
        if !eval_exprs.is_empty() && !has_main_def {
            self.emit_line("fn main() {");
            self.indent += 1;
            for expr in &eval_exprs {
                self.emit_indent();
                self.output.push_str("println!(\"{}\", ");
                self.gen_expr(expr)?;
                self.output.push_str(");\n");
            }
            self.indent -= 1;
            self.emit_line("}");
        }

        Ok(self.output.clone())
    }

    fn gen_decl(&mut self, decl: &Decl) -> LustcResult<()> {
        match decl {
            Decl::FunDef {
                name,
                params,
                return_type,
                body,
            } => self.gen_fun_def(name, params, return_type, body),
            Decl::FunDefMatch {
                name,
                params,
                return_type,
                cases,
            } => self.gen_fun_def_match(name, params, return_type, cases),
            Decl::InductiveDef { name, constructors } => self.gen_inductive(name, constructors),
            Decl::Eval(_) => Ok(()), // handled in generate()
        }
    }

    fn gen_fun_def(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Option<Type>,
        body: &Expr,
    ) -> LustcResult<()> {
        // Special case: main : IO Unit
        if name == "main" && is_io_unit(return_type) {
            self.emit_str("fn main()");
            self.output.push_str(" {\n");
            self.indent += 1;
            self.gen_do_body(body)?;
            self.indent -= 1;
            self.emit_line("}");
            return Ok(());
        }

        self.emit_str(&format!("fn {}", name));
        self.output.push('(');
        for (i, (pname, ptype)) in params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output
                .push_str(&format!("{}: {}", pname, self.type_to_rust(ptype)));
        }
        self.output.push(')');

        if let Some(ret) = return_type {
            if !matches!(ret, Type::Unit) {
                self.output
                    .push_str(&format!(" -> {}", self.type_to_rust(ret)));
            }
        }

        self.output.push_str(" {\n");
        self.indent += 1;
        self.emit_indent();
        self.gen_expr(body)?;
        self.output.push('\n');
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn gen_fun_def_match(
        &mut self,
        name: &str,
        params: &[(String, Type)],
        return_type: &Option<Type>,
        cases: &[(Vec<Pattern>, Expr)],
    ) -> LustcResult<()> {
        // For pattern-matching function defs, we need to figure out the
        // match parameter. The patterns represent additional implicit args.

        // When params is empty and return_type is Arrow, decompose:
        //   def f : A → B → C  means  fn f(x: A, y: B) -> C
        // The last arrow target is the return type, everything else is a param.
        let match_param_name = self.infer_match_param_name(cases);

        let (all_params, actual_return_type) = if params.is_empty() {
            // Decompose arrow type into parameter types + return type
            let mut arrow_parts = Vec::new();
            if let Some(ref ty) = return_type {
                Self::flatten_arrow(ty, &mut arrow_parts);
            }
            if arrow_parts.len() >= 2 {
                let ret = arrow_parts.pop().unwrap();
                let param_types: Vec<(String, String)> = arrow_parts
                    .iter()
                    .enumerate()
                    .map(|(i, t)| {
                        // Use pattern variable name for the match param (last one)
                        // Use generic names for others
                        if i == arrow_parts.len() - 1 {
                            (match_param_name.clone(), self.type_to_rust(t))
                        } else {
                            (format!("x{}", i), self.type_to_rust(t))
                        }
                    })
                    .collect();
                (param_types, Some(self.type_to_rust(ret)))
            } else {
                // Single type or no type — match param uses the single type
                let param_type = arrow_parts.first().map(|t| self.type_to_rust(t));
                let p = if let Some(ty) = param_type {
                    vec![(match_param_name.clone(), ty)]
                } else {
                    vec![]
                };
                (p, None)
            }
        } else {
            // Explicit params exist, add match param from return type
            let mut all: Vec<(String, String)> = params
                .iter()
                .map(|(n, t)| (n.clone(), self.type_to_rust(t)))
                .collect();

            // If return_type is an Arrow, decompose into extra param + real return
            if let Some(ref ty) = return_type {
                let mut arrow_parts = Vec::new();
                Self::flatten_arrow(ty, &mut arrow_parts);
                if arrow_parts.len() >= 2 {
                    let ret = arrow_parts.pop().unwrap();
                    for t in &arrow_parts {
                        all.push((match_param_name.clone(), self.type_to_rust(t)));
                    }
                    (all, Some(self.type_to_rust(ret)))
                } else {
                    let ret = return_type.as_ref().map(|t| self.type_to_rust(t));
                    all.push((match_param_name.clone(), ret.clone().unwrap_or_default()));
                    (all, ret)
                }
            } else {
                (all, None)
            }
        };

        self.emit_str(&format!("fn {}", name));
        self.output.push('(');

        for (i, (pname, ptype)) in all_params.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.output.push_str(&format!("{}: {}", pname, ptype));
        }
        self.output.push(')');

        if let Some(ref ret) = actual_return_type {
            if ret != "()" {
                self.output.push_str(&format!(" -> {}", ret));
            }
        }

        self.output.push_str(" {\n");
        self.indent += 1;

        // Generate match expression
        self.emit_indent();
        self.output
            .push_str(&format!("match {} {{\n", match_param_name));
        self.indent += 1;

        for (patterns, body) in cases {
            self.emit_indent();
            if let Some(pat) = patterns.first() {
                // Successor pattern: `n + k` → catch-all that rebinds n = n - k
                if let Pattern::Successor(var, k) = pat {
                    self.output.push_str(&format!("{} => {{\n", var));
                    self.indent += 1;
                    self.emit_indent();
                    self.output
                        .push_str(&format!("let {} = {}.saturating_sub({});\n", var, var, k));
                    self.emit_indent();
                    self.gen_expr(body)?;
                    self.output.push('\n');
                    self.indent -= 1;
                    self.emit_indent();
                    self.output.push_str("}\n");
                    continue;
                }
                self.gen_pattern(pat)?;
            }
            self.output.push_str(" => ");
            self.gen_expr(body)?;
            self.output.push_str(",\n");
        }

        self.indent -= 1;
        self.emit_indent();
        self.output.push_str("}\n");

        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn flatten_arrow<'a>(ty: &'a Type, parts: &mut Vec<&'a Type>) {
        match ty {
            Type::Arrow(lhs, rhs) => {
                parts.push(lhs.as_ref());
                Self::flatten_arrow(rhs.as_ref(), parts);
            }
            _ => {
                parts.push(ty);
            }
        }
    }

    fn infer_match_param_name(&self, cases: &[(Vec<Pattern>, Expr)]) -> String {
        for (patterns, _) in cases {
            if let Some(pat) = patterns.first() {
                match pat {
                    Pattern::Var(name) => return name.clone(),
                    Pattern::Successor(name, _) => return name.clone(),
                    _ => {}
                }
            }
        }
        "x".to_string()
    }

    fn gen_inductive(&mut self, name: &str, constructors: &[Constructor]) -> LustcResult<()> {
        self.emit_line("#[derive(Debug, Clone, PartialEq)]");
        self.emit_line(&format!("enum {} {{", name));
        self.indent += 1;
        for ctor in constructors {
            self.emit_indent();
            let pascal_name = to_pascal_case(&ctor.name);
            if ctor.fields.is_empty() {
                self.output.push_str(&format!("{},\n", pascal_name));
            } else {
                self.output.push_str(&format!("{}(", pascal_name));
                for (i, field) in ctor.fields.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(&self.type_to_rust(field));
                }
                self.output.push_str("),\n");
            }
        }
        self.indent -= 1;
        self.emit_line("}");
        Ok(())
    }

    fn gen_expr(&mut self, expr: &Expr) -> LustcResult<()> {
        match expr {
            Expr::IntLit(n) => self.output.push_str(&n.to_string()),
            Expr::StringLit(s) => {
                self.output.push_str("String::from(\"");
                self.output
                    .push_str(&s.replace('\\', "\\\\").replace('"', "\\\""));
                self.output.push_str("\")");
            }
            Expr::BoolLit(b) => self.output.push_str(if *b { "true" } else { "false" }),
            Expr::Var(name, _span) => {
                self.output.push_str(&self.translate_var(name));
            }
            Expr::BinOp { op, lhs, rhs } => {
                // Special case: Nat subtraction uses saturating_sub
                if *op == BinOp::Sub {
                    self.gen_expr(lhs)?;
                    self.output.push_str(".saturating_sub(");
                    self.gen_expr(rhs)?;
                    self.output.push(')');
                } else {
                    self.gen_expr(lhs)?;
                    self.output.push_str(&format!(" {} ", binop_to_rust(op)));
                    self.gen_expr(rhs)?;
                }
            }
            Expr::UnaryOp { op, operand } => {
                match op {
                    UnaryOp::Not => self.output.push('!'),
                    UnaryOp::Neg => self.output.push('-'),
                }
                self.gen_expr(operand)?;
            }
            Expr::FunApp { func, arg } => {
                self.gen_fun_app(func, arg)?;
            }
            Expr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.output.push_str("if ");
                self.gen_expr(cond)?;
                self.output.push_str(" { ");
                self.gen_expr(then_branch)?;
                self.output.push_str(" } else { ");
                self.gen_expr(else_branch)?;
                self.output.push_str(" }");
            }
            Expr::Let {
                name, value, body, ..
            } => {
                self.output.push_str(&format!("{{ let {} = ", name));
                self.gen_expr(value)?;
                self.output.push_str("; ");
                self.gen_expr(body)?;
                self.output.push_str(" }");
            }
            Expr::Match { scrutinee, arms } => {
                self.output.push_str("match ");
                self.gen_expr(scrutinee)?;
                self.output.push_str(" {\n");
                self.indent += 1;
                for arm in arms {
                    self.emit_indent();
                    self.gen_pattern(&arm.pattern)?;
                    self.output.push_str(" => ");
                    self.gen_expr(&arm.body)?;
                    self.output.push_str(",\n");
                }
                self.indent -= 1;
                self.emit_indent();
                self.output.push('}');
            }
            Expr::Do(stmts) => {
                self.gen_do_stmts(stmts)?;
            }
            Expr::Lambda { params, body } => {
                self.output.push('|');
                for (i, (name, _)) in params.iter().enumerate() {
                    if i > 0 {
                        self.output.push_str(", ");
                    }
                    self.output.push_str(name);
                }
                self.output.push_str("| ");
                self.gen_expr(body)?;
            }
            Expr::Paren(inner) => {
                self.output.push('(');
                self.gen_expr(inner)?;
                self.output.push(')');
            }
        }
        Ok(())
    }

    fn gen_fun_app(&mut self, func: &Expr, arg: &Expr) -> LustcResult<()> {
        // Flatten curried application: f x y → f(x, y)
        let mut args = vec![arg];
        let mut base = func;
        while let Expr::FunApp {
            func: inner_func,
            arg: inner_arg,
        } = base
        {
            args.push(inner_arg);
            base = inner_func;
        }
        args.reverse();

        // Special cases
        if let Expr::Var(name, _span) = base {
            // IO.println → println!("{}", ...)
            if name == "IO.println" {
                self.output.push_str("println!(\"{}\", ");
                self.gen_expr(args[0])?;
                self.output.push(')');
                return Ok(());
            }

            // toString → .to_string()
            if name == "toString" {
                self.gen_expr(args[0])?;
                self.output.push_str(".to_string()");
                return Ok(());
            }

            // Constructor: qualified name like Color.red → Color::Red
            let translated = self.translate_var(name);
            if translated.contains("::") {
                self.output.push_str(&translated);
                if !args.is_empty() && !matches!(args[0], Expr::Var(ref v, _) if v == "()") {
                    self.output.push('(');
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.gen_expr(a)?;
                    }
                    self.output.push(')');
                }
                return Ok(());
            }

            // Regular function call
            self.output.push_str(&translated);
            self.output.push('(');
            for (i, a) in args.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                self.gen_expr(a)?;
            }
            self.output.push(')');
            return Ok(());
        }

        // General case
        self.gen_expr(base)?;
        self.output.push('(');
        for (i, a) in args.iter().enumerate() {
            if i > 0 {
                self.output.push_str(", ");
            }
            self.gen_expr(a)?;
        }
        self.output.push(')');
        Ok(())
    }

    fn gen_do_body(&mut self, body: &Expr) -> LustcResult<()> {
        match body {
            Expr::Do(stmts) => self.gen_do_stmts(stmts),
            other => {
                self.emit_indent();
                self.gen_expr(other)?;
                self.output.push_str(";\n");
                Ok(())
            }
        }
    }

    fn gen_do_stmts(&mut self, stmts: &[DoStatement]) -> LustcResult<()> {
        for stmt in stmts {
            match stmt {
                DoStatement::Expr(expr) => {
                    self.emit_indent();
                    self.gen_expr(expr)?;
                    self.output.push_str(";\n");
                }
                DoStatement::Let(name, value) => {
                    self.emit_indent();
                    self.output.push_str(&format!("let {} = ", name));
                    self.gen_expr(value)?;
                    self.output.push_str(";\n");
                }
            }
        }
        Ok(())
    }

    fn gen_pattern(&mut self, pat: &Pattern) -> LustcResult<()> {
        match pat {
            Pattern::IntLit(n) => self.output.push_str(&n.to_string()),
            Pattern::Var(name) => self.output.push_str(name),
            Pattern::Constructor(name, args) => {
                let translated = self.translate_constructor(name);
                self.output.push_str(&translated);
                if !args.is_empty() {
                    self.output.push('(');
                    for (i, arg) in args.iter().enumerate() {
                        if i > 0 {
                            self.output.push_str(", ");
                        }
                        self.gen_pattern(arg)?;
                    }
                    self.output.push(')');
                }
            }
            Pattern::Wildcard => self.output.push('_'),
            Pattern::Successor(name, _k) => {
                // In match expressions, successor patterns become catch-all bindings
                self.output.push_str(name);
            }
        }
        Ok(())
    }

    // --- Helpers ---

    fn type_to_rust(&self, ty: &Type) -> String {
        match ty {
            Type::Named(name) => match name.as_str() {
                "Nat" => "u64".to_string(),
                "Int" => "i64".to_string(),
                "Bool" => "bool".to_string(),
                "String" => "String".to_string(),
                other => other.to_string(),
            },
            Type::Arrow(from, to) => {
                format!(
                    "fn({}) -> {}",
                    self.type_to_rust(from),
                    self.type_to_rust(to)
                )
            }
            Type::App(base, _arg) => {
                // IO Unit → () for return types, etc.
                if let Type::Named(name) = base.as_ref() {
                    if name == "IO" {
                        return "()".to_string();
                    }
                }
                self.type_to_rust(base)
            }
            Type::Unit => "()".to_string(),
        }
    }

    fn translate_var(&self, name: &str) -> String {
        // Handle qualified names for constructors
        if let Some(dot_pos) = name.find('.') {
            let type_name = &name[..dot_pos];
            let ctor_name = &name[dot_pos + 1..];
            if self.inductive_names.contains(&type_name.to_string()) {
                return format!("{}::{}", type_name, to_pascal_case(ctor_name));
            }
        }
        name.to_string()
    }

    fn translate_constructor(&self, name: &str) -> String {
        // Handle qualified constructor names like Color.red → Color::Red
        if let Some(dot_pos) = name.find('.') {
            let type_name = &name[..dot_pos];
            let ctor_name = &name[dot_pos + 1..];
            return format!("{}::{}", type_name, to_pascal_case(ctor_name));
        }
        // Unqualified constructor
        to_pascal_case(name)
    }

    fn emit_str(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
    }

    fn emit_line(&mut self, s: &str) {
        self.emit_indent();
        self.output.push_str(s);
        self.output.push('\n');
    }

    fn emit_indent(&mut self) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
    }
}

fn is_io_unit(ty: &Option<Type>) -> bool {
    match ty {
        Some(Type::App(base, arg)) => {
            let is_io = matches!(base.as_ref(), Type::Named(n) if n == "IO");
            let is_unit = matches!(arg.as_ref(), Type::Unit)
                || matches!(arg.as_ref(), Type::Named(n) if n == "Unit");
            is_io && is_unit
        }
        _ => false,
    }
}

fn binop_to_rust(op: &BinOp) -> &'static str {
    match op {
        BinOp::Add => "+",
        BinOp::Sub => "-", // Note: usually handled by saturating_sub
        BinOp::Mul => "*",
        BinOp::Div => "/",
        BinOp::Mod => "%",
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::And => "&&",
        BinOp::Or => "||",
    }
}

fn to_pascal_case(s: &str) -> String {
    let mut result = String::new();
    let mut capitalize_next = true;
    for ch in s.chars() {
        if ch == '_' {
            capitalize_next = true;
        } else if capitalize_next {
            result.push(ch.to_ascii_uppercase());
            capitalize_next = false;
        } else {
            result.push(ch);
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    fn compile(src: &str) -> String {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize().unwrap();
        let mut parser = Parser::new(tokens);
        let decls = parser.parse_program().unwrap();
        let mut codegen = CodeGen::new();
        codegen.generate(&decls).unwrap()
    }

    #[test]
    fn test_simple_function() {
        let result = compile("def add (x : Nat) (y : Nat) : Nat := x + y");
        assert!(result.contains("fn add(x: u64, y: u64) -> u64"));
    }

    #[test]
    fn test_eval_generates_main() {
        let result = compile("#eval 1 + 2");
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
    }

    #[test]
    fn test_inductive_type() {
        let result = compile("inductive Color where\n  | red\n  | green\n  | blue");
        assert!(result.contains("enum Color"));
        assert!(result.contains("Red"));
        assert!(result.contains("Green"));
        assert!(result.contains("Blue"));
        assert!(result.contains("#[derive(Debug, Clone, PartialEq)]"));
    }

    #[test]
    fn test_pascal_case() {
        assert_eq!(to_pascal_case("red"), "Red");
        assert_eq!(to_pascal_case("light_blue"), "LightBlue");
        assert_eq!(to_pascal_case("hello"), "Hello");
    }

    #[test]
    fn test_nat_subtraction_saturating() {
        let result = compile("def pred (n : Nat) : Nat := n - 1");
        assert!(result.contains("saturating_sub"));
    }

    #[test]
    fn test_io_println() {
        let result = compile("def main : IO Unit := do\n  IO.println \"Hello!\"");
        assert!(result.contains("fn main()"));
        assert!(result.contains("println!"));
        assert!(result.contains("Hello!"));
    }

    #[test]
    fn test_type_conversion() {
        let cg = CodeGen::new();
        assert_eq!(cg.type_to_rust(&Type::Named("Nat".to_string())), "u64");
        assert_eq!(cg.type_to_rust(&Type::Named("Int".to_string())), "i64");
        assert_eq!(cg.type_to_rust(&Type::Named("Bool".to_string())), "bool");
        assert_eq!(
            cg.type_to_rust(&Type::Named("String".to_string())),
            "String"
        );
        assert_eq!(cg.type_to_rust(&Type::Unit), "()");
    }
}
